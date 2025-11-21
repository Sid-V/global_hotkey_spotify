<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ref, onMounted, onUnmounted } from "vue";

const isLoggedIn = ref(false);
const errorMessage = ref("");
const isPlaying = ref(false);
const playPauseHotkey = ref('');
const nextTrackHotkey = ref('');
const prevTrackHotkey = ref('');
const volumeUpHotkey = ref('');
const volumeDownHotkey = ref('');
const isRecordingHotkey = ref('');

interface AuthResult {
  NeedsAuth?: { url: string };
  Success?: {ok: string}; // Hacky: Will always be string 'ok'
  Error?: { message: string };
}

interface Hotkeys {
  play_pause: string;
  next_track: string;
  prev_track: string;
  volume_up: string;
  volume_down: string;
}

interface SpotifyAuthEvent {
  code: string;
}

let unlistenAuthEvent: UnlistenFn | null = null;
let windowCallbackRegistered = false;
let authStatusInterval: number | null = null;

async function launchSpotifyAuthWindow(url: string) {
  try {
    await openUrl(url);
    return;
  } catch (pluginError) {
    console.warn("LaunchSpotifyAuthWindow: openUrl failed, falling back to window.open", pluginError);
  }

  const popup = window.open(url, "_blank");
  if (!popup) {
    throw new Error("Browser blocked the Spotify login popup. Please allow popups or copy/paste the URL: " + url);
  }
}

async function processAuthCode(code: string) {
  console.log("ProcessAuthCode: Received code", code);
  try {
    const authResult = await invoke<AuthResult>("handle_callback", {
      code,
    });

    console.log("ProcessAuthCode: Final result:", authResult);

    if (authResult.Success) {
      isLoggedIn.value = true;
      errorMessage.value = "";
    } else if (authResult.Error) {
      errorMessage.value = authResult.Error.message;
    }
  } catch (error) {
    console.error("ProcessAuthCode: Error handling callback:", error);
    errorMessage.value = "Failed to complete authentication. Unknown Error.";
  }
}

const windowCallbackHandler = async (event: MessageEvent) => {
  if (event.data.type === "spotify-callback" && event.data.code) {
    await processAuthCode(event.data.code);
  }
};

async function handleAuth() {
  // First check if we already have valid auth
  try {
    const authStatus = await invoke<AuthResult>("check_auth_status");
    console.log("HandleAuth: Initial auth status check: ", authStatus);
    
    if (authStatus.Success) {
      isLoggedIn.value = true;
      return; // Exit early since we're already authenticated
    }
  } catch (error) {
    //error("Error checking auth status: {error}");
    // Continue with auth flow if check fails
  }
  try {
    const result = await invoke<AuthResult>("init_auth");

    console.log("HandleAuth: Result", result);

    if (result.NeedsAuth) {
      console.log("HandleAuth: Opening auth window with URL:", result.NeedsAuth.url);
      await launchSpotifyAuthWindow(result.NeedsAuth.url);

      if (!windowCallbackRegistered) {
        window.addEventListener("message", windowCallbackHandler);
        windowCallbackRegistered = true;
      }
    }
    else if (result.Success) {
      isLoggedIn.value = true;
    } else if (result.Error) {
      errorMessage.value = result.Error.message;
    }
  } catch (error) {
    console.error("HandleAuth: Authentication failed:", error);
    errorMessage.value =
      error instanceof Error ? error.message : "Failed to connect to Spotify OAuth";
  }
}

async function handlePlayPause() {
  try {
    const result = await invoke<{ Success?: null; Error?: { message: string } }>(
      "play_pause"
    );
    
    if (result.Success) {
      isPlaying.value = !isPlaying.value;
      errorMessage.value = "";
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

async function handleVolumeUp() {
  try {
    const result = await invoke<{ Success?: null; Error?: { message: string } }>(
      "volume_control_up"
    );
    
    if (result.Error) {
      errorMessage.value = result.Error.message;
    }
  } catch (error) {
    console.error("Volume up failed:", error);
    errorMessage.value = "Failed to increase volume";
  }
}

async function handleVolumeDown() {
  try {
    const result = await invoke<{ Success?: null; Error?: { message: string } }>(
      "volume_control_down"
    );
    
    if (result.Error) {
      errorMessage.value = result.Error.message;
    }
  } catch (error) {
    console.error("Volume down failed:", error);
    errorMessage.value = "Failed to decrease volume";
  }
}

async function checkAuthStatus() {
  try {
    const result = await invoke<AuthResult>("check_auth_status");
    console.log("CheckAuthStatus: Result", result);
    
    
    if (result.Success) {
      isLoggedIn.value = true;
    } else if (result.Error) {
      console.log("CheckAuthStatus: Error", result.Error);
      isLoggedIn.value = false;
    }
  } catch (error) {
    console.error("CheckAuthStatus: Failed to check auth status", error);
    isLoggedIn.value = false;
  }
}

async function loadPersistedHotkeys() {
  try {
    const hotkeys = await invoke<Hotkeys>("return_loaded_hotkeys");
    playPauseHotkey.value = hotkeys["play_pause"] || '';
    nextTrackHotkey.value = hotkeys["next_track"] || '';
    prevTrackHotkey.value = hotkeys["prev_track"] || '';
    volumeUpHotkey.value = hotkeys["volume_up"] || '';
    volumeDownHotkey.value = hotkeys["volume_down"] || '';
  } catch (error) {
    console.error("LoadPersistedHotkeys: Failed to load hotkeys:", error);
    errorMessage.value = "Failed to load previous hotkeys or no previous hotkeys found";
  }
}

function handleKeyDown(e: KeyboardEvent, control: string) {
  e.preventDefault();
  
  // Only record if we're actively recording for this control
  if (isRecordingHotkey.value !== control) return;
  
  const keys: string[] = [];
  if (e.ctrlKey) keys.push('CTRL');
  if (e.altKey) keys.push('ALT');
  if (e.shiftKey) keys.push('SHIFT');
  
  // Add the main key if it's not a modifier
  if (!['CTRL', 'ALT', 'SHIFT'].includes(e.key)) {
    keys.push(e.key === ' ' ? 'SPACE' : e.key);
  }
  
  const hotkeyString = keys.join(' + ');
  
  // Update the appropriate hotkey
  switch (control) {
    case 'playPause':
      playPauseHotkey.value = hotkeyString;
      break;
    case 'nextTrack':
      nextTrackHotkey.value = hotkeyString;
      break;
    case 'prevTrack':
      prevTrackHotkey.value = hotkeyString;
      break;
    case 'volumeUp':
      volumeUpHotkey.value = hotkeyString;
      break;
    case 'volumeDown':
      volumeDownHotkey.value = hotkeyString;
      break;
  }
}

function startRecording(control: string) {
  isRecordingHotkey.value = control;
}

function stopRecording() {
  isRecordingHotkey.value = '';
}

async function saveHotkeys() {
  try {
    const result = await invoke<AuthResult>(
      "set_hotkeys",
      {
        playPauseHotkey: playPauseHotkey.value,
        nextTrackHotkey: nextTrackHotkey.value,
        prevTrackHotkey: prevTrackHotkey.value,
        volumeUpHotkey: volumeUpHotkey.value,
        volumeDownHotkey: volumeDownHotkey.value
      }
    );
    
    if (result.Error) {
      errorMessage.value = result.Error.message;
    }

  } catch (error) {
    console.error("Failed to save hotkeys:", error);
    errorMessage.value = "Failed to save hotkeys";
  }
}

onMounted(async () => {
  await checkAuthStatus();
  await loadPersistedHotkeys();

  unlistenAuthEvent = await listen<SpotifyAuthEvent>("spotify-auth-code", async (event) => {
    await processAuthCode(event.payload.code);
  });
  
  // Check auth status every 10 mins
  authStatusInterval = window.setInterval(async () => {
    await checkAuthStatus();
  }, 600000);
});

onUnmounted(() => {
  if (unlistenAuthEvent) {
    unlistenAuthEvent();
    unlistenAuthEvent = null;
  }

  if (windowCallbackRegistered) {
    window.removeEventListener("message", windowCallbackHandler);
    windowCallbackRegistered = false;
  }

  if (authStatusInterval) {
    clearInterval(authStatusInterval);
    authStatusInterval = null;
  }
});

</script>

<template>
  <div class="container">
    <h1>Global Hotkeys Spotify Controls</h1>

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
      
      <!-- Existing playback controls -->
      <div class="playback-controls">
        <button @click="handlePrevTrack" class="control-button">
          ⏮️ Previous
        </button>
        <button @click="handlePlayPause" class="control-button play-pause">
          ▶️⏸️ Play/Pause
        </button>
        <button @click="handleNextTrack" class="control-button">
          ⏭️ Next
        </button>
        <button @click="handleVolumeUp" class="control-button">
          🔊 Volume Up
        </button>
        <button @click="handleVolumeDown" class="control-button">
          🔊 Volume Down
        </button>
      </div>
      
      <!-- New Hotkey Configuration Form -->
      <div class="hotkey-config">
        <h3>Configure Hotkeys</h3>
        <form @submit.prevent="saveHotkeys" class="hotkey-form">
          <div class="hotkey-input-group">
            <label for="playPauseHotkey">Play/Pause:</label>
            <input
              id="playPauseHotkey"
              type="text"
              :value="playPauseHotkey"
              readonly
              :class="{ 'recording': isRecordingHotkey === 'playPause' }"
              @focus="startRecording('playPause')"
              @blur="stopRecording"
              @keydown="handleKeyDown($event, 'playPause')"
              placeholder="Click to set hotkey"
            />
          </div>
          
          <div class="hotkey-input-group">
            <label for="nextTrackHotkey">Next Track:</label>
            <input
              id="nextTrackHotkey"
              type="text"
              :value="nextTrackHotkey"
              readonly
              :class="{ 'recording': isRecordingHotkey === 'nextTrack' }"
              @focus="startRecording('nextTrack')"
              @blur="stopRecording"
              @keydown="handleKeyDown($event, 'nextTrack')"
              placeholder="Click to set hotkey"
            />
          </div>
          
          <div class="hotkey-input-group">
            <label for="prevTrackHotkey">Previous Track:</label>
            <input
              id="prevTrackHotkey"
              type="text"
              :value="prevTrackHotkey"
              readonly
              :class="{ 'recording': isRecordingHotkey === 'prevTrack' }"
              @focus="startRecording('prevTrack')"
              @blur="stopRecording"
              @keydown="handleKeyDown($event, 'prevTrack')"
              placeholder="Click to set hotkey"
            />
          </div>

          <div class="hotkey-input-group">
            <label for="volumeUpHotkey">Volume Up:</label>
            <input
              id="volumeUpHotkey"
              type="text"
              :value="volumeUpHotkey"
              readonly
              :class="{ 'recording': isRecordingHotkey === 'volumeUp' }"
              @focus="startRecording('volumeUp')"
              @blur="stopRecording"
              @keydown="handleKeyDown($event, 'volumeUp')"
              placeholder="Click to set hotkey"
            />
          </div>

          <div class="hotkey-input-group">
            <label for="volumeDownHotkey">Volume Down:</label>
            <input
              id="volumeDownHotkey"
              type="text"
              :value="volumeDownHotkey"
              readonly
              :class="{ 'recording': isRecordingHotkey === 'volumeDown' }"
              @focus="startRecording('volumeDown')"
              @blur="stopRecording"
              @keydown="handleKeyDown($event, 'volumeDown')"
              placeholder="Click to set hotkey"
            />
          </div>
          
          <button type="submit" class="save-hotkeys-button">Save Hotkeys</button>
        </form>
      </div>
      
      <p v-if="errorMessage" class="error">{{ errorMessage }}</p>
    </div>
  </div>
</template>

<style scoped>
.container {
  padding: 0;
  margin: 0;
  text-align: center;
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  background-color: #282828;
  overflow: hidden;
}

.login-section,
.logged-in {
  width: 100%;
  max-width: 600px;
  margin: 0 auto;
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

.hotkey-config {
  margin-top: 2rem;
  padding: 1rem;
  background-color: #282828;
  border-radius: 8px;
  width: 100%;
}

.hotkey-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  max-width: 400px;
  margin: 0 auto;
}

.hotkey-input-group {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 1rem;
}

.hotkey-input-group input {
  padding: 0.5rem;
  border: 1px solid #404040;
  border-radius: 4px;
  background-color: #181818;
  color: white;
  cursor: pointer;
  width: 200px;
}

.hotkey-input-group input.recording {
  border-color: #1DB954;
  background-color: #2a2a2a;
}

.save-hotkeys-button {
  background-color: #1DB954;
  color: white;
  border: none;
  padding: 0.75rem 1.5rem;
  border-radius: 1.5rem;
  font-weight: bold;
  cursor: pointer;
  margin-top: 1rem;
  align-self: center;
}

.save-hotkeys-button:hover {
  background-color: #1ed760;
}
</style>
