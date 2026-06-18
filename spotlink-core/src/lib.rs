use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::time::Duration;
use std::collections::HashMap;
use tokio::sync::{mpsc, broadcast};
use rumqttc::{MqttOptions, AsyncClient, QoS, Transport, Event, Incoming};
use std::sync::Arc;

// --- DATA FROM THE FIREHOSE ---
#[derive(Debug, Deserialize, Clone)]
pub struct PskMessage {
    #[serde(rename = "sc")]
    pub sender: Option<String>,
    #[serde(rename = "rc")]
    pub receiver: Option<String>,
    #[serde(rename = "rp")]
    pub snr: Option<i32>,
    #[serde(rename = "b")]
    pub band: Option<String>,
    #[serde(rename = "md")]
    pub mode: Option<String>,
}

// --- INTERNAL ENGINE STATE ---
#[derive(Debug, Clone)]
pub struct StationInfo {
    pub timestamp: DateTime<Utc>,
    pub snr: i32,
}

// --- DATA FOR THE FRONTENDS ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotLinkMatch {
    pub callsign: String,
    pub band: String,
    pub mode: String,
    pub sent_snr: i32,
    pub recv_snr: i32,
    pub timestamp: DateTime<Utc>,
}

// --- Engine Status Data ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStats {
    pub outgoing: usize, 
    pub incoming: usize, 
    pub matches: usize,  
}

// --- TRANSIT ENUM WRAPPER ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineEvent {
    MatchUpdate(SpotLinkMatch),
    StatsUpdate(EngineStats),
    GlobalTimeout,
}

// --- FRONTEND CONTROL TO ENGINE ---
#[derive(Debug, Clone)]
pub enum EngineCommand {
    StartTracking { callsign: String },
    StopTracking,
}

pub async fn run_engine(
    mut command_receiver: mpsc::Receiver<EngineCommand>,
    event_broadcaster: broadcast::Sender<EngineEvent>,
) {
    let mut current_callsign: Option<String> = None;
    
    let mut heard_me: HashMap<String, StationInfo> = HashMap::new();
    let mut i_heard: HashMap<String, StationInfo> = HashMap::new();
    let mut active_matches: HashMap<String, DateTime<Utc>> = HashMap::new();
    let mut last_seen_time = Utc::now();
    const TIMEOUT_SECS: i64 = 300; // 5 minutes

    // --- OUTER LOOP: Manages Session Configurations ---
    loop {
        if current_callsign.is_none() {
            match command_receiver.recv().await {
                Some(EngineCommand::StartTracking { callsign }) => {
                    current_callsign = Some(callsign.to_uppercase());
                    heard_me.clear();
                    i_heard.clear();
                    active_matches.clear();
                    last_seen_time = Utc::now();
                }
                Some(EngineCommand::StopTracking) => continue,
                None => break, 
            }
        }

        let callsign = current_callsign.as_ref().unwrap().clone();

        let unique_id = Utc::now().timestamp_millis();
        let client_id = format!("spotlink_{}_{}", callsign, unique_id);
        let mut mqtt_options = MqttOptions::new(client_id, "mqtt.pskreporter.info", 1884);
        mqtt_options.set_keep_alive(Duration::from_secs(60));

        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let client_config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        mqtt_options.set_transport(Transport::Tls(rumqttc::TlsConfiguration::Rustls(Arc::new(client_config))));

        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);

        let tx_topic = format!("pskr/filter/v2/+/+/{}/#", callsign);
        let rx_topic = format!("pskr/filter/v2/+/+/+/{}/#", callsign);

        if client.subscribe(tx_topic, QoS::AtMostOnce).await.is_err() {
            current_callsign = None;
            continue;
        }
        if client.subscribe(rx_topic, QoS::AtMostOnce).await.is_err() {
            current_callsign = None;
            continue;
        }

        // Create a heartbeat that ticks every 3 seconds
        let mut maintenance_ticker = tokio::time::interval(Duration::from_secs(3));

        // --- INNER LOOP: Processing Active Traffic ---
        loop {
            tokio::select! {
                // BRANCH A: Listen for control commands from our Frontend
                cmd_opt = command_receiver.recv() => {
                    match cmd_opt {
                        Some(EngineCommand::StartTracking { callsign: new_callsign }) => {
                            let normalized = new_callsign.to_uppercase();
                            if Some(&normalized) != current_callsign.as_ref() {
                                current_callsign = Some(normalized);
                                break; 
                            }
                        }
                        Some(EngineCommand::StopTracking) => {
                            current_callsign = None;
                            break; 
                        }
                        None => return, 
                    }
                }

                // BRANCH B: Listen for raw stream packets from PSK Reporter
                mqtt_event = eventloop.poll() => {
                    match mqtt_event {
                        Ok(Event::Incoming(Incoming::Publish(packet))) => {
                            if let Ok(parsed_msg) = serde_json::from_slice::<PskMessage>(&packet.payload) {
                                let sender = match parsed_msg.sender.as_deref() { Some(s) => s, None => continue };
                                let receiver = match parsed_msg.receiver.as_deref() { Some(r) => r, None => continue };
                                let snr = match parsed_msg.snr { Some(s) => s, None => continue };
                                let band = parsed_msg.band.as_deref().unwrap_or("??").trim().to_string();
                                let mode = parsed_msg.mode.as_deref().unwrap_or("??").trim().to_string();

                                let now = Utc::now();

                                // --- Log State Map Tracking ---
                                let info = StationInfo { timestamp: now, snr };
                                if sender == callsign {
                                    heard_me.insert(receiver.to_string(), info);
                                    last_seen_time = now;
                                } else if receiver == callsign {
                                    i_heard.insert(sender.to_string(), info);
                                    last_seen_time = now;
                                }

                                // --- Match Verification ---
                                let target = if sender == callsign { receiver } else { sender };

                                if let (Some(sent_info), Some(recv_info)) = (heard_me.get(target), i_heard.get(target)) {
                                    last_seen_time = now;
                                    active_matches.insert(target.to_string(), now);

                                    let clean_match = SpotLinkMatch {
                                        callsign: target.to_string(),
                                        band,
                                        mode,
                                        sent_snr: sent_info.snr,
                                        recv_snr: recv_info.snr,
                                        timestamp: now,
                                    };

                                    let _ = event_broadcaster.send(EngineEvent::MatchUpdate(clean_match));
                                }

                                // --- Broadcast Live Stats ---
                                let current_stats = EngineStats {
                                    outgoing: heard_me.len(),
                                    incoming: i_heard.len(),
                                    matches: active_matches.len(),
                                };
                                let _ = event_broadcaster.send(EngineEvent::StatsUpdate(current_stats));
                            }
                        }
                        Err(_) => {
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            break;
                        }
                        _ => {} 
                    }
                }

                // BRANCH C: Periodic Background Maintenance
                _ = maintenance_ticker.tick() => {
                    let now = Utc::now();

                    // --- Global Inactivity Timeout Reset ---
                    if now.signed_duration_since(last_seen_time).num_seconds() >= TIMEOUT_SECS {
                        // Only send the clear event if we actually have stale data
                        if !heard_me.is_empty() || !i_heard.is_empty() || !active_matches.is_empty() {
                            heard_me.clear();
                            i_heard.clear();
                            active_matches.clear();
                            
                            let _ = event_broadcaster.send(EngineEvent::GlobalTimeout);
                            let _ = event_broadcaster.send(EngineEvent::StatsUpdate(EngineStats {
                                outgoing: 0,
                                incoming: 0,
                                matches: 0,
                            }));
                        }
                    }

                    // --- Rolling Memory Garbage Collection ---
                    heard_me.retain(|_, info| now.signed_duration_since(info.timestamp).num_seconds() < TIMEOUT_SECS);
                    i_heard.retain(|_, info| now.signed_duration_since(info.timestamp).num_seconds() < TIMEOUT_SECS);
                    active_matches.retain(|_, ts| now.signed_duration_since(*ts).num_seconds() < TIMEOUT_SECS);

                    // --- Broadcast Live Stats ---
                    let stats = EngineStats {
                        outgoing: heard_me.len(),
                        incoming: i_heard.len(),
                        matches: active_matches.len(),
                    };
                    let _ = event_broadcaster.send(EngineEvent::StatsUpdate(stats));
                }
            }
        }
    }
}