use crate::AppState;
use rspotify::{
    model::user, prelude::*, scopes, AuthCodeSpotify, Config, Credentials, OAuth
};
use serde::Serialize;
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::PathBuf,
    sync::Once,
    thread,
};
use tauri::State;

const CLIENT_ID: &str = "919cdcc0a45d420d80f372105f5b96a0";
const CLIENT_SECRET: &str = "5f5aeaf0488a4e179f3f764c8f7a3b98";
const SPOTIFY_TOKEN_CACHE: &str = ".spotify_token.json";
static CALLBACK_SERVER: Once = Once::new(); // Only need to run the callback server once

#[derive(Serialize)]
pub enum AuthResult {
    Success { ok: String },
    NeedsAuth { url: String },
    Error { message: String },
}

pub fn init_spotify() -> AuthCodeSpotify {
    let config = Config {
        token_cached: true,
        token_refreshing: true,
        cache_path: PathBuf::from(SPOTIFY_TOKEN_CACHE),
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
pub async fn init_auth(state: State<'_, AppState>) -> Result<AuthResult, String> {
    
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
                    return Ok(AuthResult::Success {
                        ok: "ok".to_string(),
                    });
                }
                _ => {
                    // If refresh fails, proceed with new auth
                    dbg!("Refresh failed, starting new auth flow");
                }
            }
        } else {
            // Token exists and is valid
            return Ok(AuthResult::Success {
                ok: "ok".to_string(),
            });
        }
    }

    // No valid token, start new auth flow
    let url = spotify.get_authorize_url(true).unwrap();

    Ok(AuthResult::NeedsAuth {
        url: url.to_string(),
    })
}

#[tauri::command]
pub async fn handle_callback(
    state: State<'_, AppState>,
    code: String,
) -> Result<AuthResult, String> {
    println!("Received code from vuejs : {}", code.clone());

    // Get mutable access to Spotify client
    let mut spotify_lock = state.spotify.lock().await;
    let spotify = spotify_lock
        .as_mut()
        .ok_or_else(|| "Spotify client not initialized".to_string())?;

    // Request token using the authorization code
    match spotify.request_token(&code).await {
        Ok(_) => {
            println!("Successfully obtained token");
            // Successfully got token, try to cache it
            if let Some(token) = spotify.get_token().lock().await.unwrap().clone() {
                println!(
                    "Attempting to cache token to: {:?}",
                    spotify.config.cache_path
                );
                match token.write_cache(&spotify.config.cache_path) {
                    Ok(_) => println!("Successfully cached token"),
                    Err(e) => println!("Failed to cache token: {}", e),
                }
            }
            Ok(AuthResult::Success {
                ok: "ok".to_string(),
            })
        }
        Err(e) => {
            println!("Token request failed with error: {:?}", e);
            Ok(AuthResult::Error {
                message: format!("Failed to request token: {}", e),
            })
        }
    }
}

#[tauri::command]
pub async fn check_auth_status(state: State<'_, AppState>) -> Result<AuthResult, String> {
    let spotify_lock = state.spotify.lock().await;

    let spotify = spotify_lock.as_ref().unwrap();
    if let Ok(Some(token)) = spotify.read_token_cache(true).await {
        dbg!("Found token in cache: check auth status");
        *spotify.get_token().lock().await.unwrap() = Some(token.clone());

        if token.is_expired() {
            return Ok(AuthResult::Error {
                message: "Token expired".to_string(),
            });
        } else {
            return Ok(AuthResult::Success {
                ok: "ok".to_string(),
            });
        }
    }

    Ok(AuthResult::Error {
        message: "No token found".to_string(),
    })
}


//
// SPOTIFY API PLAYBACK FUNCTIONS
//
#[tauri::command]
pub async fn me(state: State<'_, AppState>) -> Result<Option<user::PrivateUser>, String> {
    let spotify = state.spotify.lock().await;
    if let Some(spotify) = &*spotify {
        spotify
            .me()
            .await
            .map_err(|e| format!("Failed to get user info: {}", e))
            .map(Some)
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn play_pause(state: State<'_, AppState>) -> Result<AuthResult, String> {
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
                    Ok(_) => Ok(AuthResult::Success {
                        ok: "ok".to_string(),
                    }),
                    Err(e) => Ok(AuthResult::Error {
                        message: format!("Playback control failed: {}", e),
                    }),
                }
            }
            Ok(None) => Ok(AuthResult::Error {
                message: "No active playback".to_string(),
            }),
            Err(e) => Ok(AuthResult::Error {
                message: format!("Failed to get playback state: {}", e),
            }),
        }
    } else {
        Ok(AuthResult::Error {
            message: "Spotify client not initialized".to_string(),
        })
    }
}

#[tauri::command]
pub async fn next_track(state: State<'_, AppState>) -> Result<AuthResult, String> {
    let spotify = state.spotify.lock().await;
    if let Some(spotify) = &*spotify {
        match spotify.next_track(None).await {
            Ok(_) => Ok(AuthResult::Success {
                ok: "ok".to_string(),
            }),
            Err(e) => Ok(AuthResult::Error {
                message: format!("Next Track failed: {}", e),
            }),
        }
    } else {
        Ok(AuthResult::Error {
            message: "No active playback".to_string(),
        })
    }
}

#[tauri::command]
pub async fn prev_track(state: State<'_, AppState>) -> Result<AuthResult, String> {
    let spotify = state.spotify.lock().await;
    if let Some(spotify) = &*spotify {
        match spotify.previous_track(None).await {
            Ok(_) => Ok(AuthResult::Success {
                ok: "ok".to_string(),
            }),
            Err(e) => Ok(AuthResult::Error {
                message: format!("Next Track failed: {}", e),
            }),
        }
    } else {
        Ok(AuthResult::Error {
            message: "No active playback".to_string(),
        })
    }
}