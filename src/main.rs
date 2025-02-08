mod packet;

use serde_json::json;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};
use std::{io, thread};
use serde::Deserialize;

const PORT: u16 = 25565;

struct ClientData {
    protocol: u32,
    port: u16,
    state: u32,

    stop_handling: bool,
    backend_offline: bool,
    backend_stream: Option<TcpStream>,
}

fn pipe_bidirectional<
    R1: Read + Send + 'static,
    W1: Write + Send + 'static,
    R2: Read + Send + 'static,
    W2: Write + Send + 'static,
>(
    mut r1: R1,
    mut w1: W1,
    mut r2: R2,
    mut w2: W2,
) -> io::Result<(u64, u64)> {
    let t1 = thread::spawn(move || io::copy(&mut r1, &mut w2));
    let t2 = thread::spawn(move || io::copy(&mut r2, &mut w1));

    Ok((t1.join().unwrap()?, t2.join().unwrap()?))
}

fn handle_packet(
    stream: &mut TcpStream,
    packet: &mut packet::Packet,
    client_data: &mut Option<ClientData>,
    reroutes: &HashMap<String, (String, u16)>,
) {
    if client_data.is_some() && client_data.as_ref().unwrap().stop_handling {
        return;
    }

    match packet.id {
        0x00 => {
            println!("Handshake packet");

            if client_data.is_none() {
                let protocol = packet.read_var_int();
                let host = packet.read_string();
                let port = packet.read_uint16();
                let state = packet.read_var_int();

                *client_data = Some(ClientData {
                    protocol,
                    port,
                    state,
                    stop_handling: false,
                    backend_offline: false,
                    backend_stream: None,
                });

                let reroute = reroutes.get(host.as_str());
                if reroute.is_some() {
                    let (backend_host, backend_port) = reroute.unwrap();

                    println!("Rerouting to {}:{}", backend_host, backend_port);

                    match TcpStream::connect((backend_host.as_str(), *backend_port)) {
                        Ok(backend_stream) => {
                            client_data.as_mut().unwrap().backend_stream = Some(backend_stream);
                            println!("Backend server connected");

                            if state != 1 {
                                client_data.as_mut().unwrap().stop_handling = true;

                                // proxy data to backend server
                                let mut backend_stream_clone = client_data
                                    .as_ref()
                                    .unwrap()
                                    .backend_stream
                                    .as_ref()
                                    .unwrap()
                                    .try_clone()
                                    .unwrap();

                                let mut builder = packet::PacketBuilder::new();
                                builder.write_var_int(0);
                                builder.write_var_int(protocol);
                                builder.write_string(
                                    backend_stream_clone
                                        .peer_addr()
                                        .unwrap()
                                        .ip()
                                        .to_string()
                                        .as_str(),
                                );
                                builder.write_uint16(port);
                                builder.write_var_int(state);

                                let response_packet = builder.build();
                                backend_stream_clone
                                    .write_all(&response_packet.body)
                                    .unwrap();
                                backend_stream_clone.flush().unwrap();

                                pipe_bidirectional(
                                    stream.try_clone().unwrap(),
                                    stream.try_clone().unwrap(),
                                    backend_stream_clone.try_clone().unwrap(),
                                    backend_stream_clone,
                                )
                                .unwrap_or((0, 0));

                                return;
                            }
                        }
                        Err(_) => {
                            client_data.as_mut().unwrap().backend_offline = true;
                            println!("Backend server is offline");
                        }
                    }
                }

                println!("Protocol: {}", protocol);
                println!("Host: {}", host);
                println!("Port: {}", port);
                println!("State: {}", state);
            } else {
                println!("MOTD Packet");

                if client_data.as_ref().unwrap().backend_stream.is_some() {
                    client_data.as_mut().unwrap().stop_handling = true;

                    let backend_host = client_data
                        .as_ref()
                        .unwrap()
                        .backend_stream
                        .as_ref()
                        .unwrap()
                        .peer_addr()
                        .unwrap()
                        .ip();

                    let mut builder = packet::PacketBuilder::new();
                    builder.write_var_int(0);
                    builder.write_var_int(client_data.as_ref().unwrap().protocol);
                    builder.write_string(backend_host.to_string().as_str());
                    builder.write_uint16(client_data.as_ref().unwrap().port);
                    builder.write_var_int(client_data.as_ref().unwrap().state);

                    let response_packet = builder.build();
                    let backend_stream = client_data
                        .as_mut()
                        .unwrap()
                        .backend_stream
                        .as_mut()
                        .unwrap();

                    backend_stream.write_all(&response_packet.body).unwrap();

                    let mut builder2 = packet::PacketBuilder::new();
                    builder2.write_var_int(0);

                    let response_packet2 = builder2.build();

                    backend_stream.write_all(&response_packet2.body).unwrap();
                    backend_stream.flush().unwrap();

                    pipe_bidirectional(
                        stream.try_clone().unwrap(),
                        stream.try_clone().unwrap(),
                        backend_stream.try_clone().unwrap(),
                        backend_stream.try_clone().unwrap(),
                    )
                    .unwrap_or((0, 0));

                    return;
                }

                let response = json!({
                    "version": {
                        "protocol": client_data.as_ref().unwrap().protocol,
                        "name": "Reroute Proxy",
                        "supportedVersions": [client_data.as_ref().unwrap().protocol],
                    },
                    "players": {
                        "max": 100,
                        "online": 69,
                        "sample": [],
                    },
                    "description": {
                        "text": client_data.as_ref().unwrap().backend_offline
                            .then(|| "Reroute Proxy - Backend is offline")
                            .unwrap_or("Reroute Proxy - No Server found"),
                    }
                });

                let mut builder = packet::PacketBuilder::new();
                builder.write_var_int(0);
                builder.write_string(response.to_string().as_str());

                let response_packet = builder.build();
                stream.write_all(&response_packet.body).unwrap();
            }

            return;
        }
        0x01 => {
            println!("Ping packet");

            let payload = packet.read_long();

            let mut builder = packet::PacketBuilder::new();
            builder.write_var_int(1);
            builder.write_long(payload);

            let response_packet = builder.build();
            stream.write_all(&response_packet.body).unwrap();

            return;
        }
        _ => {
            println!("Unknown packet: {}", packet.id);
        }
    }
}

#[derive(Deserialize)]
struct Reroutes {
    reroutes: HashMap<String, (String, u16)>,
}

fn main() {
    let file = std::fs::File::open("reroutes.json").unwrap();
    let data: Reroutes = serde_json::from_reader(file).unwrap();

    let listener = TcpListener::bind(("0.0.0.0", PORT)).unwrap();
    println!("Server started on port {}", PORT);
    println!("{} Reroutes active", data.reroutes.len());

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let mut client_data: Option<ClientData> = None;

                println!("New connection: {}", stream.peer_addr().unwrap());

                loop {
                    let mut buffer = [0; 1024];
                    let bytes_read = stream.read(&mut buffer).unwrap();

                    if bytes_read == 0 {
                        break;
                    }

                    let mut packet = packet::Packet::new(buffer[..bytes_read].to_vec());
                    handle_packet(&mut stream, &mut packet, &mut client_data, &data.reroutes);

                    let offset = packet.offset as usize;

                    // slice the buffer to remove the processed packet
                    buffer.copy_within(offset..bytes_read, 0);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    drop(listener);

    println!("Server stopped");
}
