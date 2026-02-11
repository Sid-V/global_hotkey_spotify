# Quick Start - Security Fixes Testing Guide

## Build & Test Commands

### 1. Check Compilation
```bash
cd src-tauri
cargo check
```

### 2. Build Release
```bash
cargo build --release
```

### 3. Run Development Server
```bash
cd ..
pnpm tauri dev
```

---

## Manual Testing Steps

### Test #1: CSP Policy
1. Open app
2. Open browser DevTools (if using dev mode)
3. Login with Spotify
4. Check console for CSP violations (should be none)
5. Verify all features work (hotkeys, playback controls)

### Test #2: Environment Variable
```bash
# Test 1: Without env var (uses default)
cd src-tauri
cargo build

# Test 2: With env var (uses custom)
set SPOTIFY_CLIENT_ID=your_custom_id
cargo build
```

### Test #3: File Permissions (Unix/macOS)
```bash
# After logging in and caching token:
ls -la ~/.cache/com.global-hotkey-spotify.app/.spotify_token.json

# Expected output:
# -rw------- 1 username username ... .spotify_token.json
#     ^^^
#     Should be 600 (owner read/write only)
```

### Test #4: File Permissions (Windows)
```powershell
# After logging in:
icacls "%LOCALAPPDATA%\com.global-hotkey-spotify.app\.spotify_token.json"

# Should show only current user has access
```

### Test #5: WSA Initialization (Windows)
1. Run app on Windows
2. Check logs for:
   - "WSAStartup succeeded"
   - "Callback_server: listening on port 8888"
3. Complete OAuth flow
4. Close app
5. Check logs for "WSACleanup succeeded"

### Test #6: Port Conflict Handling
```bash
# Start something on port 8888:
python -m http.server 8888

# Then start the app
# Should see error: "Failed to bind to 127.0.0.1:8888"
# App should continue running (graceful failure)
```

### Test #7: Origin Validation
1. Open app
2. Open browser DevTools
3. In console, try:
   ```javascript
   window.postMessage({
     type: 'spotify-callback',
     code: 'fake-code-12345'
   }, '*');
   ```
4. Should see: "Rejected message from untrusted origin"

---

## Expected Log Messages

### Successful Startup
```
[INFO] Setting up Tauri...
[INFO] App cache dir: ...
[INFO] App data dir (spotify token cache): ...
[INFO] Windows Sockets initialized successfully  # Windows only
[INFO] Callback_server: listening on port 8888
[INFO] registered for autostart? true
[INFO] Tauri setup complete!
```

### After Login (Unix/macOS)
```
[DEBUG] Handle_callback: Successfully cached token
[INFO] Set secure permissions (0600) on token cache: ...
```

### After Login (Windows)
```
[DEBUG] Handle_callback: Successfully cached token
[INFO] Token cache file permissions rely on Windows app data directory security: ...
```

---

## Troubleshooting

### Issue: "SPOTIFY_CLIENT_ID not set" warning
**Solution:** This is expected. The default Client ID will be used. To suppress:
```bash
export SPOTIFY_CLIENT_ID=919cdcc0a45d420d80f372105f5b96a0  # Unix/macOS
set SPOTIFY_CLIENT_ID=919cdcc0a45d420d80f372105f5b96a0     # Windows
```

### Issue: CSP violation errors
**Solution:** Report these - they indicate the CSP policy needs adjustment.

### Issue: Token file wrong permissions
**Solution:** 
1. Delete `.spotify_token.json`
2. Logout and login again
3. Permissions should be set correctly on new file

### Issue: "Failed to bind to 127.0.0.1:8888"
**Causes:**
- Port already in use by another app
- Firewall blocking port 8888
- Permission issues

**Solution:**
1. Check what's using port 8888: `netstat -ano | findstr 8888` (Windows) or `lsof -i :8888` (Unix)
2. Close the conflicting app
3. Restart the app

---

## Success Criteria

✅ All checkboxes passed:
- [ ] App compiles without errors
- [ ] App launches successfully
- [ ] OAuth login works
- [ ] Token cached with secure permissions
- [ ] No CSP violations in console
- [ ] All hotkeys work
- [ ] Playback controls work
- [ ] No crashes or panics

---

## Files to Review

### Modified Files
1. `src-tauri/tauri.conf.json` - CSP policy
2. `src-tauri/build.rs` - Env var handling
3. `src-tauri/Cargo.toml` - Winapi features
4. `src-tauri/src/api.rs` - Major security improvements
5. `src/App.vue` - Origin validation
6. `.gitignore` - Env file patterns

### New Files
7. `.env.example` - Environment template
8. `SECURITY_REVIEW.md` - Complete security review
9. `CRITICAL_FIXES_PLAN.md` - Implementation plan
10. `IMPLEMENTATION_SUMMARY.md` - What was done
11. `TESTING_GUIDE.md` - This file

---

## Reporting Issues

If you encounter any problems:
1. Check logs in app data directory
2. Note exact error message
3. Include OS and version
4. Include steps to reproduce
5. Check if issue exists before the security fixes

---

## Next Steps After Testing

1. ✅ Verify all tests pass
2. Update `SECURITY_REVIEW.md` with completion status
3. Commit changes with descriptive message:
   ```bash
   git add .
   git commit -m "Security: Fix 4 critical security issues

   - Add CSP policy to prevent XSS
   - Move Client ID to environment variable
   - Secure token cache with file permissions
   - Fix unsafe WSA initialization on Windows
   - Add postMessage origin validation

   Closes #[issue-numbers]"
   ```
4. Create pull request
5. Request security-focused code review
6. Merge after approval
7. Create release notes highlighting security improvements

---

**Created:** 2026-02-11  
**For:** Critical Security Fixes Implementation  
**Estimated Testing Time:** 2-4 hours
