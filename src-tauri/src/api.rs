use rspotify::{
    model::user, 
    prelude::*, 
    scopes, AuthCodeSpotify, Config, Credentials, OAuth, Token,
};
use tauri::{State, Url};
use std::str::FromStr;
use std::{env, fs, path::PathBuf, io, collections::HashMap};
use serde::Serialize;
use warp::Filter;
use tokio::sync::Mutex;
use std::net::TcpListener;
use std::thread;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::sync::Once;

const CLIENT_ID: &str = "919cdcc0a45d420d80f372105f5b96a0";
const CLIENT_SECRET: &str = "5f5aeaf0488a4e179f3f764c8f7a3b98";

// Add this at the top with other statics
static CALLBACK_SERVER: Once = Once::new();

#[derive(Serialize)]
pub enum AuthResult {
    Success { ok: String },
    NeedsAuth { url: String },
    Error { message: String },
}

// Main state of the app
pub struct SpotifyAuthState {
    spotify: Mutex<Option<AuthCodeSpotify>>,
}

impl Default for SpotifyAuthState {
    fn default() -> Self {
        Self {
            spotify: Mutex::new(Some(init_spotify())),            
        }
    }
}

fn init_spotify() -> AuthCodeSpotify {
    let config = Config {
        token_cached: true,
        token_refreshing: true,
        cache_path: PathBuf::from(".spotify_token.json"),
        ..Default::default()
    };
    
    let api_scopes = scopes!(
        "user-read-email",
        "user-read-private",
        "user-read-recently-played",
        "user-library-read",
        "user-read-currently-playing",
        "user-read-playback-state",
        "user-read-playback-position",
        "user-modify-playback-state"
    );
    
    let creds = Credentials::new(CLIENT_ID, CLIENT_SECRET);
    
    let oauth = OAuth {
        scopes: api_scopes,
        redirect_uri: "http://localhost:8888/callback".to_owned(),
        ..Default::default()
    };

    AuthCodeSpotify::with_config(creds, oauth, config)
}

fn start_callback_server() {
    CALLBACK_SERVER.call_once(|| {
        thread::spawn(|| {
            let listener = TcpListener::bind("127.0.0.1:8888").unwrap();
            println!("Callback server listening on port 8888");
            
            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        // Read the request to get the URL with code
                        let buf_reader = BufReader::new(&stream);
                        let request_line = buf_reader.lines().next();
                        
                        if let Some(Ok(line)) = request_line {
                            println!("Received request: {}", line);
                            
                            let response = format!("HTTP/1.1 200 OK\r\n\
                                Content-Type: text/html\r\n\
                                Access-Control-Allow-Origin: *\r\n\
                                \r\n\
                                <html><body><script>\
                                console.log('Callback page loaded');\
                                const urlParams = new URLSearchParams(window.location.search);\
                                const code = urlParams.get('code');\
                                if (window.opener && code) {{\
                                    console.log('Sending code to opener:', code);\
                                    window.opener.postMessage({{ type: 'spotify-callback', code: code }}, '*');\
                                    window.close();\
                                }}\
                                </script>\
                                <p>Authentication successful! You can close this window.</p>\
                                </body></html>");
                            
                            stream.write_all(response.as_bytes()).unwrap();
                        }
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
            }
        });
    });
}

#[tauri::command]
pub async fn init_auth(state: State<'_, SpotifyAuthState>) -> Result<AuthResult, String> {

    start_callback_server();
    
    let spotify_lock = state.spotify.lock().await;
    
    let spotify = spotify_lock.as_ref().unwrap();
    // Check for existing token
    if let Ok(Some(token)) = spotify.read_token_cache(true).await {
        dbg!("Found token in cache: init_auth");
        *spotify.get_token().lock().await.unwrap() = Some(token.clone());
        
        if token.is_expired() {
            dbg!("Token expired, attempting refresh");
            match spotify.refresh_token().await {
                Ok(()) => {
                    dbg!("Token refreshed successfully");
                    return Ok(AuthResult::Success { ok: "ok".to_string() });
                }
                _ => {
                    // If refresh fails, proceed with new auth
                    dbg!("Refresh failed, starting new auth flow");
                }
            }
        } else {
            // Token exists and is valid
            return Ok(AuthResult::Success { ok: "ok".to_string() });
        }
    }

    // No valid token, start auth flow
    let url = spotify.get_authorize_url(true).unwrap();
    
    
    Ok(AuthResult::NeedsAuth { url: url.to_string()})
}

#[tauri::command]
pub async fn handle_callback(
    state: State<'_, SpotifyAuthState>,
    code: String,
) -> Result<AuthResult, String> {
    println!("Received code from vuejs : {}", code.clone());

    // Get mutable access to Spotify client
    let mut spotify_lock = state.spotify.lock().await;
    let spotify = spotify_lock.as_mut()
        .ok_or_else(|| "Spotify client not initialized".to_string())?;

    // Request token using the authorization code
    match spotify.request_token(&code).await {
        Ok(_) => {
            println!("Successfully obtained token");
            // Successfully got token, try to cache it
            if let Some(token) = spotify.get_token().lock().await.unwrap().clone() {
                println!("Attempting to cache token to: {:?}", spotify.config.cache_path);
                match token.write_cache(&spotify.config.cache_path) {
                    Ok(_) => println!("Successfully cached token"),
                    Err(e) => println!("Failed to cache token: {}", e)
                }
            }
            Ok(AuthResult::Success { ok: "ok".to_string() })
        },
        Err(e) => {
            println!("Token request failed with error: {:?}", e);
            Ok(AuthResult::Error { 
                message: format!("Failed to request token: {}", e) 
            })
        }
    }
}

#[tauri::command]
pub async fn check_auth_status(state: State<'_, SpotifyAuthState>) -> Result<AuthResult, String> {
    let spotify_lock = state.spotify.lock().await;
    
    let spotify = spotify_lock.as_ref().unwrap();
    if let Ok(Some(token)) = spotify.read_token_cache(true).await {
        dbg!("Found token in cache: check auth status");
        *spotify.get_token().lock().await.unwrap() = Some(token.clone());
        
        if token.is_expired() {
            return Ok(AuthResult::Error { 
                message: "Token expired".to_string() 
            });
        } else {
            return Ok(AuthResult::Success { ok: "ok".to_string() });
        }
    }
    
    Ok(AuthResult::Error { message: "No token found".to_string() })

}

// #[tauri::command]
// pub async fn handle_callback(
//     state: State<'_, SpotifyAuthState>, 
//     callback_url: String,
// ) -> Result<String, String> {
//     // Extract code from callback URL
//     let parsed_url = Url::parse(&callback_url).map_err(|e| format!("Invalid URL: {}", e))?;
//     let code = parsed_url.query_pairs()
//         .find(|(key, _)| key == "code")
//         .map(|(_, value)| value.to_string())
//         .ok_or_else(|| "Missing code parameter".to_string())?;

//     // Get Spotify client and request token
//     let mut spotify_lock = state.spotify.lock().await;
//     let spotify = spotify_lock
//         .as_mut()
//         .ok_or_else(|| "Spotify client not initialized".to_string())?;

//     spotify
//         .request_token(&code)
//         .await
//         .map_err(|e| format!("Failed to request token: {}", e))?;

//     // Cache the token
//     if let Some(token) = spotify.token.lock().await.unwrap().clone() {
//         token.write_cache(&spotify.config.cache_path)
//             .map_err(|e| format!("Failed to write token to cache: {}", e))?;
//     }

//     Ok("Successfully authenticated".to_string())
// }


#[tauri::command]
pub async fn me(state: State<'_, SpotifyAuthState>) -> Result<Option<user::PrivateUser>, String> {
    
    let spotify = state.spotify.lock().await;
    if let Some(spotify) = &*spotify {
        spotify.me().await
            .map_err(|e| format!("Failed to get user info: {}", e))
            .map(Some)
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn play_pause(state: State<'_, SpotifyAuthState>) -> Result<AuthResult, String> {
    
    let spotify = state.spotify.lock().await;
    if let Some(spotify) = &*spotify {
        match spotify.current_playback(None, None::<Vec<_>>).await {
            Ok(Some(playback)) => {
                let result = if playback.is_playing {
                    spotify.pause_playback(None).await
                } else {
                    spotify.resume_playback(None, None).await
                };
                
                match result {
                    Ok(_) => Ok(AuthResult::Success { ok: "ok".to_string() }),
                    Err(e) => Ok(AuthResult::Error {
                        message: format!("Playback control failed: {}", e)
                    })
                }
            },
            Ok(None) => Ok(AuthResult::Error {
                message: "No active playback".to_string()
            }),
            Err(e) => Ok(AuthResult::Error {
                message: format!("Failed to get playback state: {}", e)
            })
        }
    } else {
        Ok(AuthResult::Error {
            message: "Spotify client not initialized".to_string()
        })
    }
}

#[tauri::command]
pub async fn next_track(state: State<'_, SpotifyAuthState>) -> Result<AuthResult, String> {    

    let spotify = state.spotify.lock().await;
    if let Some(spotify) = &*spotify {
        match spotify.next_track(None).await {
            Ok(_) => Ok(AuthResult::Success { ok: "ok".to_string() }),
            Err(e) => Ok(AuthResult::Error { message: format!("Next Track failed: {}", e) })
        }
    } else {
        Ok(AuthResult::Error {
            message: "No active playback".to_string()
        })
    }
}

#[tauri::command]
pub async fn prev_track(state: State<'_, SpotifyAuthState>) -> Result<AuthResult, String> {
    
    let spotify = state.spotify.lock().await;
    if let Some(spotify) = &*spotify {
        match spotify.previous_track(None).await {
            Ok(_) => Ok(AuthResult::Success { ok: "ok".to_string() }),
            Err(e) => Ok(AuthResult::Error { message: format!("Next Track failed: {}", e) })
        }
    } else {
        Ok(AuthResult::Error {
            message: "No active playback".to_string()
        })
    }
}


// Helper Functions

// fn is_authenticated(jar: &CookieJar) -> bool {
//     if jar.get("uuid").is_none() || !cache_path_exists(jar) {
//         return false;
//     }

//     let cache_path = get_cache_path(jar);
//     match Token::from_cache(&cache_path) {
//         Ok(token) => !token.is_expired(),
//         Err(_) => false,
//     }
// }

// fn get_cache_path(jar: &CookieJar) -> PathBuf {
//     let mut cache_path = env::current_dir().unwrap_or_default();
//     cache_path.push(CACHE_PATH);
    
//     if let Some(uuid) = jar.get("uuid") {
//         cache_path.push(uuid.value());
//     }
    
//     cache_path
// }

// fn cache_path_exists(jar: &CookieJar) -> bool {
//     get_cache_path(jar).exists()
// }

// fn create_cache_path_if_absent(jar: &CookieJar) -> PathBuf {
//     let cache_path = get_cache_path(jar);
//     if !cache_path.exists() {
//         if let Some(parent) = cache_path.parent() {
//             let _ = fs::create_dir_all(parent);
//         }
//     }
//     cache_path
// }

// fn generate_random_uuid(length: usize) -> String {
//     let alphanum: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
//     let mut buf = vec![0u8; length];
//     if getrandom(&mut buf).is_ok() {
//         buf.iter()
//             .map(|byte| alphanum[*byte as usize % alphanum.len()] as char)
//             .collect()
//     } else {
//         // Fallback to a timestamp-based ID if getrandom fails
//         use std::time::{SystemTime, UNIX_EPOCH};
//         SystemTime::now()
//             .duration_since(UNIX_EPOCH)
//             .unwrap_or_default()
//             .as_nanos()
//             .to_string()
//     }
// }