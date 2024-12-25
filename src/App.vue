<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { ref, onMounted } from "vue";

const isLoggedIn = ref(false);
const errorMessage = ref("");
const isPlaying = ref(false);
const currentTrack = ref("");

interface AuthResult {
  NeedsAuth?: { url: string };
  Success?: {ok: string}; // Hacky: Will always be 'ok'
  Error?: { message: string };
}

async function handleAuth() {
  // First check if we already have valid auth
  try {
    const authStatus = await invoke<AuthResult>("check_auth_status");
    console.log("Initial auth status check:", authStatus);
    
    if (authStatus.Success) {
      isLoggedIn.value = true;
      return; // Exit early since we're already authenticated
    }
  } catch (error) {
    console.error("Error checking auth status:", error);
    // Continue with auth flow if check fails
  }
  try {
    const result = await invoke<AuthResult>("init_auth");

    console.log("result", result);

    if (result.NeedsAuth) {
      console.log("Opening auth window with URL:", result.NeedsAuth.url);
      const authWindow = window.open(result.NeedsAuth.url, "_blank");
      
      // Listen for the code from the callback window
      window.addEventListener('message', async function handleCallback(event) {
        console.log("Received message:", event.data);
        
        if (event.data.type === 'spotify-callback' && event.data.code) {
          console.log("Received callback code:", event.data.code);
          
          try {
            const authResult = await invoke<AuthResult>("handle_callback", {
              code: event.data.code
            });
            
            console.log("Auth result:", authResult);
            
            if (authResult.Success) {
              isLoggedIn.value = true;
            } else if (authResult.Error) {
              errorMessage.value = authResult.Error.message;
            }
          } catch (error) {
            console.error("Error handling callback:", error);
            errorMessage.value = "Failed to complete authentication";
          }
        }
      });
    }
    else if (result.Success) {
      isLoggedIn.value = true;
    } else if (result.Error) {
      errorMessage.value = result.Error.message;
    }
  } catch (error) {
    console.error("Authentication failed:", error);
    console.error("Error occurred on line:", new Error().stack?.split('\n')[1]?.trim());
    errorMessage.value = "Failed to connect to Spotify";
  }
}

async function handlePlayPause() {
  try {
    const result = await invoke<{ Success?: null; Error?: { message: string } }>(
      "play_pause"
    );
    
    if (result.Success) {
      isPlaying.value = !isPlaying.value;
    } else if (result.Error) {
      errorMessage.value = result.Error.message;
    }
  } catch (error) {
    console.error("Playback control failed:", error);
    errorMessage.value = "Failed to control playback";
  }
}

async function handlePrevTrack() {
  try {
    const result = await invoke<{ Success?: null; Error?: { message: string } }>(
      "prev_track"
    );
    
    if (result.Error) {
      errorMessage.value = result.Error.message;
    }
  } catch (error) {
    console.error("Previous track failed:", error);
    errorMessage.value = "Failed to play previous track";
  }
}

async function handleNextTrack() {
  try {
    const result = await invoke<{ Success?: null; Error?: { message: string } }>(
      "next_track"
    );
    
    if (result.Error) {
      errorMessage.value = result.Error.message;
    }
  } catch (error) {
    console.error("Next track failed:", error);
    errorMessage.value = "Failed to play next track";
  }
}

async function checkAuthStatus() {
  try {
    const result = await invoke<AuthResult>("check_auth_status");
    console.log("Auth status check result:", result);
    
    if (result.Success) {
      isLoggedIn.value = true;
    } else if (result.Error) {
      console.log("Auth check error:", result.Error);
      isLoggedIn.value = false;
    }
  } catch (error) {
    console.error("Failed to check auth status:", error);
    isLoggedIn.value = false;
  }
}

onMounted(async () => {
  await checkAuthStatus();
  
  // Check auth status every 5 mins
  setInterval(async () => {
    await checkAuthStatus();
  }, 300000);
});

</script>

<template>
  <div class="container">
    <h1>Spotify Tauri App</h1>

    <!-- Login Section -->
    <div v-if="!isLoggedIn" class="login-section">
      <h2>Welcome!</h2>
      <p>Connect your Spotify account to get started</p>
      <button @click="handleAuth" class="login-button">
        Login with Spotify
      </button>
      <p v-if="errorMessage" class="error">{{ errorMessage }}</p>
    </div>

    <!-- Logged-In Section with Playback Controls -->
    <div v-else class="logged-in">
      <h2>Spotify Player</h2>
      <div class="playback-controls">
        <button @click="handlePrevTrack" class="control-button">
          ⏮️ Previous
        </button>
        <button @click="handlePlayPause" class="control-button play-pause">
          {{ isPlaying ? "⏸️ Pause" : "▶️ Play" }}
        </button>
        <button @click="handleNextTrack" class="control-button">
          ⏭️ Next
        </button>
      </div>
      <p v-if="errorMessage" class="error">{{ errorMessage }}</p>
    </div>
  </div>
</template>

<style scoped>
.container {
  padding: 2rem;
  text-align: center;
}

.login-section {
  margin-top: 2rem;
}

.login-button {
  background-color: #1DB954;
  color: white;
  border: none;
  padding: 1rem 2rem;
  border-radius: 2rem;
  font-weight: bold;
  cursor: pointer;
  margin-top: 1rem;
}

.login-button:hover {
  background-color: #1ed760;
}

.error {
  color: red;
  margin-top: 1rem;
}

.playback-controls {
  display: flex;
  justify-content: center;
  gap: 1rem;
  margin-top: 2rem;
}

.control-button {
  background-color: #1DB954;
  color: white;
  border: none;
  padding: 0.75rem 1.5rem;
  border-radius: 1.5rem;
  font-weight: bold;
  cursor: pointer;
  transition: background-color 0.2s;
}

.control-button:hover {
  background-color: #1ed760;
}

.play-pause {
  min-width: 120px;
}
</style>
