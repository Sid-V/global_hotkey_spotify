use cookie::{Cookie, CookieJar, SameSite};
use cookie::time::Duration;
use rocket::http::Status;
use rspotify::{
    model::{user, AdditionalType, Country, Device, Market, TimeRange}, 
    prelude::*, 
    scopes, AuthCodeSpotify, Config, Credentials, OAuth, Token,
};
use getrandom::getrandom;
use tauri::State;
use std::{collections::HashMap, env, fs, path::PathBuf, io};


const CACHE_PATH: &str = ".hotkey_rspotify_cache/";
const CLIENT_ID: &str = "919cdcc0a45d420d80f372105f5b96a0";
const CLIENT_SECRET: &str = "5f5aeaf0488a4e179f3f764c8f7a3b98";

#[derive(Default)]
pub struct SpotifyAuthState {
    spotify: Mutex<Option<AuthCodeSpotify>>,
    cookie_jar: Mutex<CookieJar>,
}

pub impl Default for SpotifyAuthState {
    fn default() -> Self {
        Self {
            spotify: Mutex::new(None),
            cookie_jar: Mutex::new(CookieJar::new()),
        }
    }
}


pub fn init_spotify(jar:  &mut CookieJar) -> AuthCodeSpotify {

    let config = Config {
        token_cached : true,
        cache_path : create_cache_path_if_absent(jar),
        ..Default::default()
    };

    //println!("Created cache path: {}",{config.cache_path.display().to_string()});
    
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
    
    // TODO - GET FROM ENV
    // let creds = Credentials::from_env().unwrap();
    // let oauth = OAuth::from_env(scopes).unwrap();
    // AuthCodeSpotify::with_config(creds, oauth, config);

    let creds = Credentials::new(CLIENT_ID, CLIENT_SECRET);
    
    let oauth = OAuth {
        scopes: api_scopes,
        redirect_uri: "http://localhost:8888/callback".to_owned(),
        ..Default::default()
    };

    AuthCodeSpotify::with_config(creds, oauth, config)
    
}



#[tauri::command]
pub async fn init_auth(state: State<'_, SpotifyAuthState> ) -> Result<Status, String> {

    let mut jar = state.cookie_jar.lock().map_err(|e| e.to_string())?;
    // The user is authenticated if their cookie is set and a cache exists for them.
    if !is_authenticated(&jar)  {
        let uuid = Cookie::build(("uuid", generate_random_uuid(64)))
            .path("/")
            .secure(true)
            .max_age(Duration::minutes(60))
            .same_site(SameSite::Lax)
            .build();

        jar.add_original(uuid);
        let spotify = init_spotify(&mut jar);
        let auth_url = spotify.get_authorize_url(true).unwrap();
    }

    let cache_path = get_cache_path(jar);
    let token = Token::from_cache(cache_path).unwrap();
    // Refresh token if token is expired
    if token.is_expired() {
        let spotify = init_spotify(&mut jar);
        *spotify.token.lock().unwrap() = Some(token);
        match spotify.refresh_token() {
            Ok(_) => {
                dbg!("Successfully refreshed token");
                Status::Ok
            }
            Err(err) => {
                dbg!("Error in refreshing token in init_auth");
                Status::InternalServerError
            }
        }
    } else {
        let spotify = AuthCodeSpotify::from_token(token);
        Status::Ok
    }

}

#[tauri::command]
pub fn handle_callback() {}



pub fn me(jar: &CookieJar) -> Status {
    if !is_authenticated(jar) {
        println!("cannot access jar : me");
        return Status::Unauthorized;
    }

    let cache_path = get_cache_path(jar);
    match Token::from_cache(cache_path) {
        Ok(token) => {
            let spotify = AuthCodeSpotify::from_token(token);
            match spotify.me() {
                Ok(user_info) => {
                    println!("User info: {:?}", user_info.display_name);
                    
                     return Status::Ok;
                }
                Err(_) => return Status::InternalServerError,
            }
        }
        Err(err) => {
            return Status::InternalServerError;
        }
    }
}

pub fn top_artists(jar: &CookieJar) -> Status {
    if !is_authenticated(jar) {
        println!("cannot access jar : top artists");
        return Status::Unauthorized;
    }

    let cache_path = get_cache_path(jar);
    match Token::from_cache(cache_path) {
        Ok(token) => {
            let spotify = AuthCodeSpotify::from_token(token);
            let top_artists = spotify
                .current_user_top_artists(Some(TimeRange::LongTerm))
                .take(10)
                .filter_map(Result::ok)
                .collect::<Vec<_>>();

            for artist in top_artists {
                println!("{:?}", artist.name);
            }
                
                return Status::Ok
        }
        Err(err) => {
            return Status::InternalServerError;
        }
    }
}

pub fn next_track(jar: &CookieJar) -> Status {
    if !is_authenticated(jar) {
        println!("cannot access jar : next_track");
        return Status::Unauthorized;
    }
    
    let cache_path = get_cache_path(jar);
    match Token::from_cache(cache_path) {
        Ok(token) => {
            let spotify = AuthCodeSpotify::from_token(token);
            match spotify.next_track(None) {
                Ok(_) => {
                    println!("Next track...");
                     return Status::Ok;
                }
                Err(_) => return Status::InternalServerError,
            }
        }
        Err(err) => {
            return Status::InternalServerError;
        }
    }
}

pub fn prev_track(jar: &CookieJar) -> Status {
    if !is_authenticated(jar) {
        println!("cannot access jar : prev_track");
        return Status::Unauthorized;
    }
    
    let cache_path = get_cache_path(jar);
    match Token::from_cache(cache_path) {
        Ok(token) => {
            let spotify = AuthCodeSpotify::from_token(token);
            match spotify.previous_track(None) {
                Ok(_) => {
                    println!("Previous track...");
                     return Status::Ok;
                }
                Err(_) => return Status::InternalServerError,
            }
        }
        Err(err) => {
            return Status::InternalServerError;
        }
    }
}

pub fn play_pause(jar: &CookieJar) -> Status {
    
    if !is_authenticated(jar) {
        println!("cannot access jar : play_pause");
        return Status::Unauthorized;
    }
    
    let cache_path = get_cache_path(jar);
    match Token::from_cache(cache_path) {
        Ok(token) => {
            let spotify = AuthCodeSpotify::from_token(token);
            match spotify.current_playback(None, None::<Vec<_>>) {
                Ok(curr_context) => {
                    // TODO - if nothing is playing then this unwrap will fail
                    let curr_context = curr_context.unwrap();
                    println!("Is it playing? : {}", curr_context.is_playing);
                    
                    if curr_context.is_playing {
                        match spotify.pause_playback(None) {
                            Ok(_) => {
                                println!("Pausing music...");
                            },
                            Err(_) => {
                                println!("Error pausing music");
                            }

                        }
                    } else {
                        match spotify.resume_playback(None, None) {
                            Ok(_) => {
                                println!("Resuming music...");
                            },
                            Err(_) => {
                                println!("Error resuming music");
                            }
                        }
                        
                    }
                    println!("play pause done...");
                     return Status::Ok;
                }
                Err(_) => return Status::InternalServerError,
            }
        }
        Err(err) => {
            return Status::InternalServerError;
        }
    }
}

// HELPER FUNCTIONS

fn is_authenticated(jar: &CookieJar) -> bool {
    let authenticated = jar.get("uuid").is_some() && cache_path_exists(jar);
    if authenticated {
        let cache_path = get_cache_path(jar);
        match Token::from_cache(cache_path) {
            Ok(token) => {
                if token.is_expired() {
                    refresh_token(&mut jar.clone())
                } else {
                    true
                }
            }
            Err(_) => false,
        }
    } else {
        false
    }
}

fn refresh_token(jar: &CookieJar) -> bool {
    let cache_path = get_cache_path(jar);
    let token = Token::from_cache(cache_path).unwrap();
    if token.is_expired() {
        let spotify = init_spotify(&mut jar.clone());
        *spotify.token.lock().unwrap() = Some(token);
        match spotify.refresh_token() {
            Ok(_) => {
                println!("Successfully refreshed token");
                true
            }
            Err(err) => {
                println!("Failed to refresh token");
                false
            }
        }
    } else {
        true
    }
}

fn get_cache_path(jar: &CookieJar) -> PathBuf {
    let mut cache_path = env::current_dir().unwrap();
    cache_path.push(CACHE_PATH);
    cache_path.push(jar.get("uuid").unwrap().value());

    cache_path
}

fn cache_path_exists(jar: &CookieJar) -> bool {
    let cache_path = get_cache_path(jar);
    cache_path.exists()
}

fn create_cache_path_if_absent(jar: &CookieJar) -> PathBuf {
    let cache_path = get_cache_path(jar);
    if !cache_path.exists() {
        let mut path = cache_path.clone();
        path.pop();
        fs::create_dir_all(path).unwrap();
    }
    cache_path
}

/// Generate `length` random chars
fn generate_random_uuid(length: usize) -> String {
    let alphanum: &[u8] =
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".as_bytes();
    let mut buf = vec![0u8; length];
    getrandom(&mut buf).unwrap();
    let range = alphanum.len();

    buf.iter()
        .map(|byte| alphanum[*byte as usize % range] as char)
        .collect()
}