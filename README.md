# Global Hotkeys Spotify
This application will control your Spotify playback using Global Hotkeys. Cross-platform due to Tauri.

![image](https://github.com/user-attachments/assets/7376b75f-b42a-4529-84d6-39297598f10c)

Playback controls
- Play/Pause
- Next Track
- Prev Track
- Volume Up
- Volume Down
- Toast notification (Future)

Volume up/down controls the spotify in-app volume and NOT the windows/mac system slider volume

## Why did I build this?
I wanted to learn a new language - Rust. I used to use this github project called - Toastify that did the exact same thing but Spotify updated their Auth loop and the original author did not continue to maintain it. I didn't want to fork that project since it was written in C#.

The main usecase is to be able to control your music quickly without changing the in-focus application. Especially useful when you are playing videogames and want to skip tracks or decrease the volume since a new round is starting.

## "Why not use the media playback keys on your keyboard?"
I have a 60% keyboard and do not have playback keys nor do I want to install VIA or an equivalent and map a bunch of layers to my keyboard. I wanted to learn Rust and build an app that I WANT.

## Installation
Go to Releases and install the msi package. Now open 'global-hotkey-spotify'

## Usage
- Login using your spotify credentials. Please ignore the initial error message that says 'failed to load hotkeys'.
- Test your spotify credentials are working using the buttons that play/pause, next track, prev track, volume up/down. 

NOTE: THERE MUST BE AN ACTIVE PLAYBACK FOR IT TO WORK. SO IF IT DOESN'T WORK, OPEN SPOTIFY AND PLAY A SONG.

- Now, add your global hotkey combinations and hit Save
- Enjoy!

## Hotkeys usable
You can use either 0, 1 or a maximum of 2 modifiers. Modifiers are CTRL, ALT, CMD (on mac), SHIFT
The hotkeys you can use are 
- ALL DIGITS
- ALL LETTERS
- The following special chars:-
- "-"
- "="
- "/"
- "\\"
- ";"
- "'"
- ","
- "."
- "["
- "]"
- "`"
- "Home"
- "End"
- "PageUp"
- "PageDown"
- "Delete"
- "Backspace"
- "Escape"
- "Tab"
- "PrintScreen"
- "ScrollLock"
- "Pause"
- "Insert"
- "NumLock"
- "F1" 
- "F2" 
- "F3" 
- "F4" 
- "F5" 
- "F6" 
- "F7" 
- "F8" 
- "F9" 
- "F10"
- "F11"
- "F12"
- "F13"
- "F14"
- "F15"
- "F16"
- "F17"
- "F18"
- "F19"
- "F20"

## Testing
I've only tested on my windows machine for all functionality. This app should be cross platform since it's built on Tauri but ymmv. Please add an issue if you have any bugs, I'm happy to fix them :) Or better, you can create a PR!
  

Please report any issues, this is still super early in development and I'm trying to learn Rust with this project
