use jni::JNIEnv;
use jni::objects::{JObject, JString, GlobalRef, JClass};
use jni::sys::{jstring, jint};
use jni::JavaVM;
use sha1::{Sha1, Digest};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use serde::{Deserialize, Serialize};

#[macro_use]
extern crate lazy_static;

#[derive(Deserialize, Debug)]
struct Pool {
    name: String,
    ip: String,
    port: u16,
}

#[derive(Serialize, Clone, Debug)]
struct DebugInfo {
    is_mining: bool,
    thread_count: usize,
    connection_status: String,
    last_error: String,
}

impl Default for DebugInfo {
    fn default() -> Self {
        DebugInfo {
            is_mining: false,
            thread_count: 0,
            connection_status: "Not Connected".to_string(),
            last_error: "None".to_string(),
        }
    }
}


lazy_static! {
    static ref MINING_FLAG: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref THREAD_HANDLES: Arc<Mutex<Vec<thread::JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));
    static ref DEBUG_INFO: Arc<Mutex<DebugInfo>> = Arc::new(Mutex::new(DebugInfo::default()));
}

fn get_pool() -> Result<Pool, reqwest::Error> {
    let response: serde_json::Value = reqwest::blocking::get("https://server.duinocoin.com/getPool")?.json()?;
    let pool = Pool {
        name: response["name"].as_str().unwrap_or_default().to_string(),
        ip: response["ip"].as_str().unwrap_or_default().to_string(),
        port: response["port"].as_str().unwrap_or_default().parse::<u16>().unwrap_or(2813),
    };
    Ok(pool)
}

fn call_on_mining_event(env: &mut JNIEnv, service: &JObject, message: &str) {
    let message = env.new_string(message).expect("Couldn't create java string!");
    env.call_method(
        service,
        "onMiningEvent",
        "(Ljava/lang/String;)V",
        &[(&message).into()],
    ).expect("Failed to call onMiningEvent");
}

fn mine(thread_id: u32, username: String, vm: Arc<JavaVM>, service_ref: GlobalRef) {
    let mut env = vm.attach_current_thread().unwrap();
    let service = service_ref.as_obj();

    loop {
        if !*MINING_FLAG.lock().unwrap() { break; }

        let pool = match get_pool() {
            Ok(p) => p,
            Err(e) => {
                let error_msg = format!("[Thread {}] Error getting pool: {}", thread_id, e);
                DEBUG_INFO.lock().unwrap().last_error = error_msg.clone();
                call_on_mining_event(&mut env, &service, &error_msg);
                thread::sleep(Duration::from_secs(10));
                continue;
            }
        };

        DEBUG_INFO.lock().unwrap().connection_status = format!("Connecting to {}", pool.name);
        let server_address = format!("{}:{}", pool.ip, pool.port);
        let mut stream = match TcpStream::connect_timeout(&server_address.parse().unwrap(), Duration::from_secs(10)) {
            Ok(s) => s,
            Err(e) => {
                let error_msg = format!("[Thread {}] Error connecting to server: {}", thread_id, e);
                DEBUG_INFO.lock().unwrap().last_error = error_msg.clone();
                call_on_mining_event(&mut env, &service, &error_msg);
                thread::sleep(Duration::from_secs(10));
                continue;
            }
        };
        DEBUG_INFO.lock().unwrap().connection_status = format!("Connected to {}", pool.name);
        stream.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(15))).unwrap();

        let mut buffer = [0; 128];
        if stream.read(&mut buffer).is_err() {
             let error_msg = format!("[Thread {}] Error reading server version", thread_id);
             DEBUG_INFO.lock().unwrap().last_error = error_msg.clone();
             call_on_mining_event(&mut env, &service, &error_msg);
             continue;
        }

        while *MINING_FLAG.lock().unwrap() {
            let job_request = format!("JOB,{},LOW\n", username);
            if stream.write_all(job_request.as_bytes()).is_err() { break; }

            let mut job_buffer = [0; 1024];
            let job_str = match stream.read(&mut job_buffer) {
                Ok(n) if n > 0 => String::from_utf8_lossy(&job_buffer[..n]).trim_end_matches('\n').to_string(),
                _ => break,
            };

            let job_parts: Vec<&str> = job_str.split(',').collect();
            if job_parts.len() < 3 { continue; }

            let last_block_hash = job_parts[0];
            let expected_hash_bytes = match hex::decode(job_parts[1]) { Ok(bytes) => bytes, Err(_) => continue };
            let difficulty = job_parts[2].parse::<u64>().unwrap_or(100);
            let start_time = std::time::Instant::now();
            let mut hasher = Sha1::new();
            hasher.update(last_block_hash.as_bytes());

            for nonce in 0..=(difficulty * 100 + 1) {
                if !*MINING_FLAG.lock().unwrap() { break; }

                let mut hasher_clone = hasher.clone();
                hasher_clone.update(nonce.to_string().as_bytes());
                let result_hash = hasher_clone.finalize();

                if result_hash[..] == expected_hash_bytes[..] {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let hashrate = if elapsed > 0.0 { nonce as f64 / elapsed } else { 0.0 };

                    let result_submission = format!("{},{},Android Miner (Rust)\n", nonce, hashrate);
                    if stream.write_all(result_submission.as_bytes()).is_err() { break; }

                    let mut feedback_buffer = [0; 128];
                    if let Ok(n) = stream.read(&mut feedback_buffer) {
                        let feedback = String::from_utf8_lossy(&feedback_buffer[..n]).trim().to_string();
                        call_on_mining_event(&mut env, &service, &format!("[{}] {} {:.2} H/s", thread_id, feedback, hashrate));
                    }
                    break;
                }
            }
        }
        DEBUG_INFO.lock().unwrap().connection_status = "Disconnected".to_string();
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_example_duco_1miner_MiningService_startMining(
    mut env: JNIEnv,
    service: JObject,
    username: JString,
    cores: jint,
    threads: jint,
) {
    let mut mining_flag = MINING_FLAG.lock().unwrap();
    if *mining_flag { return; }
    *mining_flag = true;

    let num_threads = (cores * threads) as u32;
    {
        let mut debug_info = DEBUG_INFO.lock().unwrap();
        debug_info.is_mining = true;
        debug_info.thread_count = num_threads as usize;
    }
    drop(mining_flag);

    let username: String = env.get_string(&username).unwrap().into();
    let vm = Arc::new(env.get_java_vm().unwrap());
    let service_ref = env.new_global_ref(service).unwrap();

    let mut handles = THREAD_HANDLES.lock().unwrap();
    for i in 0..num_threads {
        let username = username.clone();
        let vm = vm.clone();
        let service_ref = service_ref.clone();

        let handle = thread::spawn(move || {
            mine(i, username, vm, service_ref);
        });
        handles.push(handle);
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_example_duco_1miner_MiningService_stopMining(
    mut env: JNIEnv,
    service: JObject,
) {
    let mut mining_flag = MINING_FLAG.lock().unwrap();
    if *mining_flag {
        *mining_flag = false;

        {
            let mut debug_info = DEBUG_INFO.lock().unwrap();
            debug_info.is_mining = false;
        }

        let service_ref = env.new_global_ref(service).unwrap();
        let vm = Arc::new(env.get_java_vm().unwrap());

        thread::spawn(move || {
            let mut env = vm.attach_current_thread().unwrap();
            let service = service_ref.as_obj();

            let mut handles = THREAD_HANDLES.lock().unwrap();
            for handle in handles.drain(..) {
                let _ = handle.join();
            }
            call_on_mining_event(&mut env, &service, "STOPPED");
        });
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_example_duco_1miner_MainActivity_getDebugInfo(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let debug_info = DEBUG_INFO.lock().unwrap();
    let json_string = serde_json::to_string_pretty(&*debug_info).unwrap_or_else(|e| {
        format!("{{ \"error\": \"Failed to serialize debug info: {}\" }}", e)
    });

    let output = env
        .new_string(json_string)
        .expect("Couldn't create java string!");

    output.into_raw()
}