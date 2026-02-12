use rspotify::{prelude::*, scopes, AuthCodePkceSpotify, Config, Credentials, OAuth};
use serde::Serialize;
use std::{
    io::{BufRead, BufReader, Write}, net::TcpListener, path::{Path, PathBuf}, sync::Once, thread
};
use tauri::{AppHandle, Emitter, State};
use urlencoding::decode;

#[cfg(target_os = "windows")]
use winapi::um::winsock2::{WSAStartup, WSACleanup, WSADATA};
#[cfg(target_os = "windows")]
use winapi::shared::minwindef::WORD;
#[cfg(target_os = "windows")]
use winapi::shared::winerror::NO_ERROR;

#[cfg(target_os = "windows")]
fn makeword(low: u8, high: u8) -> WORD {
    ((high as WORD) << 8) | (low as WORD)
}

// RAII wrapper for Windows Sockets initialization
#[cfg(target_os = "windows")]
struct WsaGuard {
    _initialized: bool,
}

#[cfg(target_os = "windows")]
impl WsaGuard {
    fn new() -> Result<Self, std::io::Error> {
        use std::mem::MaybeUninit;
        
        unsafe {
            let mut wsa_data: MaybeUninit<WSADATA> = MaybeUninit::uninit();
            let result = WSAStartup(makeword(2, 2), wsa_data.as_mut_ptr());
            
            if result != NO_ERROR as i32 {
                log::error!("WSAStartup failed with error: {}", result);
                return Err(std::io::Error::from_raw_os_error(result));
            }
            
            log::debug!("WSAStartup succeeded");
            Ok(Self { _initialized: true })
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for WsaGuard {
    fn drop(&mut self) {
        unsafe {
            let result = WSACleanup();
            if result != 0 {
                log::warn!("WSACleanup failed with error: {}", result);
            } else {
                log::debug!("WSACleanup succeeded");
            }
        }
    }
}

use crate::AppState;
static CALLBACK_SERVER: Once = Once::new(); // Only need to run the callback server once

#[cfg(target_os = "windows")]
use std::sync::OnceLock;
#[cfg(target_os = "windows")]
static WSA_GUARD: OnceLock<WsaGuard> = OnceLock::new();

#[derive(Serialize, Clone)]
struct SpotifyAuthPayload {
    code: String,
}

const CLIENT_ID: &str = env!("SPOTIFY_CLIENT_ID");
const SPOTIFY_TOKEN_CACHE: &str = ".spotify_token.json";

// Secure file permissions for token cache
#[cfg(unix)]
fn set_secure_permissions(path: &Path) -> std::io::Result<()> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    
    let metadata = fs::metadata(path)?;
    let mut permissions = metadata.permissions();
    
    // Set permissions to 0600 (owner read/write only)
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions)?;
    
    log::info!("Set secure permissions (0600) on token cache: {:?}", path);
    Ok(())
}

#[cfg(target_os = "windows")]
fn set_secure_permissions(path: &Path) -> std::io::Result<()> {
    
    // Note: This is a simplified implementation
    // A full implementation would create a proper DACL with only current user access
    // For now, we rely on the file being created in the user's app data directory
    // which already has restricted access on Windows
    
    log::info!("Token cache file permissions rely on Windows app data directory security: {:?}", path);
    Ok(())
}

#[cfg(not(any(unix, target_os = "windows")))]
fn set_secure_permissions(path: &Path) -> std::io::Result<()> {
    log::warn!("Secure file permissions not implemented for this platform: {:?}", path);
    Ok(())
}

#[derive(Serialize)]
pub enum AuthResult {
    Success { ok: String },
    NeedsAuth { url: String },
    Error { message: String },
}

pub fn init_spotify(cache_dir: PathBuf) -> AuthCodePkceSpotify {
    
    log::info!("Init_Spotify: Initializing spotify oauth (PKCE) object");
    log::info!("Init_Spotify: Token cache path: {:?}", cache_dir.join(SPOTIFY_TOKEN_CACHE));
    
    let config = Config {
        token_cached: true,
        token_refreshing: true,
        cache_path: cache_dir.join(SPOTIFY_TOKEN_CACHE),
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

    let creds = Credentials::new_pkce(CLIENT_ID);

    let oauth = OAuth {
        scopes: api_scopes,
        redirect_uri: "http://127.0.0.1:8888/callback".to_owned(),
        ..Default::default()
    };

    AuthCodePkceSpotify::with_config(creds, oauth, config)
}

fn start_callback_server(app_handle: AppHandle) {
    CALLBACK_SERVER.call_once(move || {
        #[cfg(target_os = "windows")]
        {
            // Initialize Windows Sockets with proper error handling
            match WsaGuard::new() {
                Ok(guard) => {
                    if WSA_GUARD.set(guard).is_err() {
                        log::error!("WSA_GUARD already initialized");
                        return;
                    }
                    log::info!("Windows Sockets initialized successfully");
                }
                Err(e) => {
                    log::error!("Failed to initialize Windows Sockets: {}", e);
                    log::error!("Callback server will not start. OAuth may not work.");
                    return;
                }
            }
        }
        
        let thread_app_handle = app_handle.clone();
        thread::spawn(move || {
            let app_handle = thread_app_handle;
            
            // Bind with proper error handling
            let listener = match TcpListener::bind("127.0.0.1:8888") {
                Ok(l) => l,
                Err(e) => {
                    log::error!("Callback_server: Failed to bind to 127.0.0.1:8888: {}", e);
                    log::error!("OAuth callback server will not work. Port may be in use or blocked.");
                    return;
                }
            };
            
            log::info!("Callback_server: listening on port 8888");

            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        // Read the request to get the URL with code
                        let buf_reader = BufReader::new(&stream);
                        let request_line = buf_reader.lines().next();

                        if let Some(Ok(line)) = request_line {
                            log::debug!("Callback_server: Received callback request");

                            if let Some(code) = extract_code_from_request_line(&line) {
                                log::debug!("Callback_server: Extracted auth code");
                                if let Err(e) = app_handle.emit(
                                    "spotify-auth-code",
                                    SpotifyAuthPayload { code: code.clone() },
                                ) {
                                    log::error!("Callback_server: Failed to emit auth code: {}", e);
                                } else {
                                    log::debug!("Callback_server: Emitted auth code event to frontend");
                                }
                            } else {
                                log::warn!("Callback_server: Unable to parse auth code from request line: {}", line);
                            }

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

                            if let Err(e) = stream.write_all(response.as_bytes()) {
                                log::warn!("Callback_server: Failed to write response to stream: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Callback_server: Error: {}", e);
                    }
                }
            }
        });
    });
}

fn extract_code_from_request_line(request_line: &str) -> Option<String> {
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    if method != "GET" {
        return None;
    }

    let path_and_query = parts.next()?;
    let (path, query) = path_and_query.split_once('?')?;
    if !path.starts_with("/callback") {
        return None;
    }

    for kv in query.split('&') {
        let (key, value) = kv.split_once('=')?;
        if key == "code" {
            return decode(value).ok().map(|c| c.into_owned());
        }
    }

    None
}

#[tauri::command]
pub async fn init_auth(app_handle: tauri::AppHandle, state: State<'_, AppState>) -> Result<AuthResult, String> {
    start_callback_server(app_handle);

    log::debug!("Init_Auth: Called");

    let mut spotify_lock = state.spotify.lock().await;
    let spotify = spotify_lock.as_mut().unwrap();
    // Check for existing token
    if let Ok(Some(token)) = spotify.read_token_cache(true).await {
        
        log::debug!("Init_Auth: Existing token found in Init_Auth");
        
        *spotify.get_token().lock().await.unwrap() = Some(token.clone());

        if token.is_expired() {
            log::debug!("Init_Auth: Token expired, attempting refresh");
            match spotify.refresh_token().await {
                Ok(()) => {
                    log::debug!("Init_Auth: Token refreshed successfully");
                    return Ok(AuthResult::Success {
                        ok: "ok".to_string(),
                    });
                }
                _ => {
                    // If refresh fails, proceed with new auth
                    log::debug!("Init_Auth: Token refresh failed. Starting new auth flow");
                }
            }
        } else {
            // Token exists and is valid
            return Ok(AuthResult::Success {
                ok: "ok".to_string(),
            });
        }
    }

    // No valid token, start new auth flow (PKCE: get_authorize_url needs &mut self to store verifier)
    let url = spotify.get_authorize_url(None).unwrap();

    Ok(AuthResult::NeedsAuth {
        url: url.to_string(),
    })
}

#[tauri::command]
pub async fn handle_callback(
    state: State<'_, AppState>,
    code: String,
) -> Result<AuthResult, String> {
    
    log::debug!("Handle_callback: Received code from frontend vue!");

    let mut spotify_lock = state.spotify.lock().await;
    let spotify = spotify_lock
        .as_mut()
        .ok_or_else(|| "Handle_callback: Spotify client not initialized".to_string())?;

    match spotify.request_token(&code).await {
        Ok(_) => {
            log::debug!("Handle_callback: Successfully requested token");
            // Successfully got token, try to cache it
            if let Some(token) = spotify.get_token().lock().await.unwrap().clone() {
                
                log::debug!("Handle_callback: Attempting to cache token to: {:?}", spotify.config.cache_path);
                match token.write_cache(&spotify.config.cache_path) {
                    Ok(_) => {
                        log::debug!("Handle_callback: Successfully cached token");
                        
                        // Set secure file permissions on token cache
                        if let Err(e) = set_secure_permissions(&spotify.config.cache_path) {
                            log::warn!("Handle_callback: Failed to set secure permissions on token cache: {}", e);
                            // Continue - not critical enough to fail the auth flow
                        }
                    }
                    Err(e) => log::error!("Handle_callback: Failed to cache token: {}", e),
                }
            }

            // Try to get the current playback state
            match spotify.current_playback(None, None::<Vec<_>>).await {
                Ok(Some(playback)) => {
                    let mut volume_lock = state.volume.lock().await;
                    *volume_lock = playback.device.volume_percent.unwrap_or(50) as u8;
                    log::debug!("Handle_callback: Current playback state volume: {:?}", *volume_lock);
                }
                Ok(None) => {
                    log::debug!("Handle_callback: No active playback");
                }
                Err(e) => {
                    log::error!("Handle_callback: Failed to get playback state: {}", e);
                }
            }

            Ok(AuthResult::Success {
                ok: "ok".to_string(),
            })
        }
        Err(e) => {
            log::error!("Handle_callback: Token request failed with error: {:?}", e);
            Ok(AuthResult::Error {
                message: format!("Handle_callback: Failed to request token: {}", e),
            })
        }
    }
}

#[tauri::command]
pub async fn check_auth_status(state: State<'_, AppState>) -> Result<AuthResult, String> {
    let spotify_lock = state.spotify.lock().await;

    let spotify = spotify_lock.as_ref().unwrap();
    if let Ok(Some(token)) = spotify.read_token_cache(true).await {
        log::debug!("Check_Auth_Status: Found token in cache");
        *spotify.get_token().lock().await.unwrap() = Some(token.clone());

        if token.is_expired() {
            return Ok(AuthResult::Error {
                message: "Check_Auth_Status: Token expired".to_string(),
            });
        } else {
            return Ok(AuthResult::Success {
                ok: "ok".to_string(),
            });
        }
    }

    Ok(AuthResult::Error {
        message: "Check_Auth_Status: No token found".to_string(),
    })
}

//
// SPOTIFY API PLAYBACK FUNCTIONS
//

#[tauri::command]
pub async fn play_pause(state: State<'_, AppState>) -> Result<AuthResult, String> {
    log::debug!("Play_Pause: Called");
    let spotify = state.spotify.lock().await;
    if let Some(spotify) = &*spotify {
        match spotify.current_playback(None, None::<Vec<_>>).await {
            Ok(Some(playback)) => {
                let mut volume_lock = state.volume.lock().await;
                *volume_lock = playback.device.volume_percent.unwrap_or(50) as u8;
                log::debug!("Play_Pause: Current playback volume percent: {:?}", *volume_lock);
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
                        message: format!("Play_Pause: Playback control API failed: {}", e),
                    }),
                }
            }
            Ok(None) => Ok(AuthResult::Error {
                message: "Play_Pause: No active playback".to_string(),
            }),
            Err(e) => Ok(AuthResult::Error {
                message: format!("Play_Pause: Failed to get playback state: {}", e),
            }),
        }
    } else {
        Ok(AuthResult::Error {
            message: "Play_Pause: Spotify client not initialized".to_string(),
        })
    }
}

#[tauri::command]
pub async fn next_track(state: State<'_, AppState>) -> Result<AuthResult, String> {
    log::debug!("Next_Track: Called");
    let spotify = state.spotify.lock().await;
    if let Some(spotify) = &*spotify {
        match spotify.next_track(None).await {
            Ok(_) => Ok(AuthResult::Success {
                ok: "ok".to_string(),
            }),
            Err(e) => Ok(AuthResult::Error {
                message: format!("Next Track: API failed: {}", e),
            }),
        }
    } else {
        Ok(AuthResult::Error {
            message: "Next_track: No active playback".to_string(),
        })
    }
}

#[tauri::command]
pub async fn prev_track(state: State<'_, AppState>) -> Result<AuthResult, String> {
    log::debug!("Prev_Track: Called");
    let spotify = state.spotify.lock().await;
    if let Some(spotify) = &*spotify {
        match spotify.previous_track(None).await {
            Ok(_) => Ok(AuthResult::Success {
                ok: "ok".to_string(),
            }),
            Err(e) => Ok(AuthResult::Error {
                message: format!("Prev_Track: API failed: {}", e),
            }),
        }
    } else {
        Ok(AuthResult::Error {
            message: "Prev_Track: No active playback".to_string(),
        })
    }
}

#[tauri::command]
pub async fn volume_control_up(state: State<'_, AppState>) -> Result<AuthResult, String> {
    log::debug!("Volume_Control_Up: Called");
    let spotify = state.spotify.lock().await;
    let mut volume_lock = state.volume.lock().await;
    if let Some(spotify) = &*spotify {
        log::info!("Volume_Control_Up: Current volume: {:?} | Setting to: {:?}", *volume_lock, (*volume_lock + 10).min(100));
        *volume_lock = (*volume_lock + 10).min(100); // Increase volume by 10, max 100
        match spotify.volume(*volume_lock, None).await {
            Ok(_) => Ok(AuthResult::Success {
                ok: "ok".to_string(),
            }),
            Err(e) => Ok(AuthResult::Error {
                message: format!("Volume_Control_Up: API failed: {}", e),
            }),
        }
    } else {
        Ok(AuthResult::Error {
            message: "Volume_Control_Up: No active playback".to_string(),
        })
    }
}

#[tauri::command]
pub async fn volume_control_down(state: State<'_, AppState>) -> Result<AuthResult, String> {
    log::debug!("Volume_Control_Down: Called");
    let spotify = state.spotify.lock().await;
    let mut volume_lock = state.volume.lock().await;
    if let Some(spotify) = &*spotify {
        log::info!("Volume_Control_Down: Current volume: {:?} | Setting to: {:?}", *volume_lock, (*volume_lock as i8 - 10).max(0) as u8);  
        *volume_lock = (*volume_lock as i8 - 10).max(0) as u8; // Decrease volume by 10, min 0
        match spotify.volume(*volume_lock, None).await {
            Ok(_) => Ok(AuthResult::Success {
                ok: "ok".to_string(),
            }),
            Err(e) => Ok(AuthResult::Error {
                message: format!("Volume_Control_Down: API failed: {}", e),
            }),
        }
    } else {
        Ok(AuthResult::Error {
            message: "Volume_Control_Down: No active playback".to_string(),
        })
    }
}
