# Critical Security Issues - Implementation Plan

**Project:** Global Hotkey Spotify  
**Date Created:** 2026-02-11  
**Priority:** CRITICAL - Address within 1 week  
**Estimated Total Time:** 12-16 hours

---

## Overview

This plan addresses the 4 critical security issues identified in the security review:
1. Exposed Client ID in source code
2. NULL CSP (Content Security Policy)
3. Insecure token cache permissions
4. Unsafe memory operations in Windows WSA initialization

---

## 🔴 Issue #1: Exposed Client ID in Source Code

### Current State
```rust
// src-tauri/src/api.rs:27
const CLIENT_ID: &str = "919cdcc0a45d420d80f372105f5b96a0";
```

### Risk Level
**CRITICAL** - Client ID can be scraped, rate-limited, or abused

### Implementation Plan

#### Step 1.1: Create Environment Variable Support
**File:** `src-tauri/build.rs`
**Action:** Create if doesn't exist, add environment variable handling

```rust
fn main() {
    // Read CLIENT_ID from environment at build time
    let client_id = std::env::var("SPOTIFY_CLIENT_ID")
        .unwrap_or_else(|_| {
            println!("cargo:warning=SPOTIFY_CLIENT_ID not set, using default");
            "919cdcc0a45d420d80f372105f5b96a0".to_string()
        });
    
    println!("cargo:rustc-env=SPOTIFY_CLIENT_ID={}", client_id);
    
    tauri_build::build()
}
```

#### Step 1.2: Update api.rs to Use Environment Variable
**File:** `src-tauri/src/api.rs`
**Action:** Replace hardcoded CLIENT_ID

```rust
// OLD:
const CLIENT_ID: &str = "919cdcc0a45d420d80f372105f5b96a0";

// NEW:
const CLIENT_ID: &str = env!("SPOTIFY_CLIENT_ID");
```

#### Step 1.3: Create .env.example
**File:** `.env.example` (new file in root)

```bash
# Spotify OAuth Configuration
# Get your Client ID from: https://developer.spotify.com/dashboard
SPOTIFY_CLIENT_ID=your_client_id_here
```

#### Step 1.4: Update .gitignore
**File:** `.gitignore`
**Action:** Add .env to gitignore if not present

```
# Environment variables
.env
.env.local
```

#### Step 1.5: Update Documentation
**File:** `README.md`
**Action:** Add section on environment setup

```markdown
## Development Setup

1. Clone the repository
2. Copy `.env.example` to `.env`
3. Add your Spotify Client ID to `.env`:
   ```bash
   SPOTIFY_CLIENT_ID=your_actual_client_id
   ```
4. Run `pnpm install`
5. Run `pnpm tauri dev`
```

#### Testing
- [ ] Build with environment variable set
- [ ] Build without environment variable (should use default)
- [ ] Verify CLIENT_ID is not in compiled binary: `strings target/release/global-hotkey-spotify.exe | grep "919cdcc0"`
- [ ] Test OAuth flow still works

**Estimated Time:** 2-3 hours  
**Dependencies:** None  
**Risk:** Low - fallback to default if env var not set

---

## 🔴 Issue #2: NULL CSP (Content Security Policy)

### Current State
```json
// tauri.conf.json
"security": {
  "csp": null
}
```

### Risk Level
**HIGH** - No protection against XSS, code injection, or unauthorized resource loading

### Implementation Plan

#### Step 2.1: Define Strict CSP Policy
**File:** `tauri.conf.json`
**Action:** Replace null CSP with strict policy

```json
"security": {
  "csp": "default-src 'self'; connect-src 'self' https://api.spotify.com https://accounts.spotify.com; img-src 'self' data: https:; style-src 'self' 'unsafe-inline'; script-src 'self' 'wasm-unsafe-eval'"
}
```

**CSP Breakdown:**
- `default-src 'self'` - Only allow resources from the app itself
- `connect-src 'self' https://api.spotify.com https://accounts.spotify.com` - Allow API calls to Spotify
- `img-src 'self' data: https:` - Allow images from app, data URIs, and HTTPS
- `style-src 'self' 'unsafe-inline'` - Allow inline styles (needed for Vue)
- `script-src 'self' 'wasm-unsafe-eval'` - Allow scripts from app and WASM

#### Step 2.2: Test CSP with Application
**Actions:**
1. Build application with new CSP
2. Test all features:
   - Login flow
   - Spotify API calls
   - Hotkey configuration
   - Tray icon
   - Window management

#### Step 2.3: Adjust CSP if Needed
If any features break, adjust CSP directives:
- Check browser console for CSP violations
- Add necessary exceptions with comments explaining why
- Document any `'unsafe-*'` directives

#### Step 2.4: Add CSP Reporting (Optional Enhancement)
For production monitoring, consider adding CSP reporting:

```json
"csp": "default-src 'self'; connect-src 'self' https://api.spotify.com https://accounts.spotify.com; report-uri /csp-violation-report"
```

Then implement a Tauri command to log violations.

#### Testing
- [ ] App builds successfully with CSP
- [ ] Login flow works
- [ ] All API calls succeed
- [ ] Hotkey configuration works
- [ ] No CSP errors in console
- [ ] External resource loading blocked (test with `<img src="http://evil.com/bad.jpg">`)

**Estimated Time:** 1-2 hours  
**Dependencies:** None  
**Risk:** Medium - May need to iterate on CSP policy

---

## 🔴 Issue #3: Insecure Token Cache Permissions

### Current State
Token cache file `.spotify_token.json` created with default permissions (readable by all users)

### Risk Level
**MEDIUM-HIGH** - Access tokens could be stolen by other users on the system

### Implementation Plan

#### Step 3.1: Add Platform-Specific File Permissions (Unix/Linux/macOS)
**File:** `src-tauri/src/api.rs`
**Action:** Set secure permissions after writing token cache

```rust
use std::fs;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

// Add new function
#[cfg(unix)]
fn set_secure_permissions(path: &std::path::Path) -> std::io::Result<()> {
    let metadata = fs::metadata(path)?;
    let mut permissions = metadata.permissions();
    
    // Set permissions to 0600 (owner read/write only)
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions)?;
    
    log::info!("Set secure permissions (0600) on: {:?}", path);
    Ok(())
}

#[cfg(not(unix))]
fn set_secure_permissions(path: &std::path::Path) -> std::io::Result<()> {
    // On Windows, use ACLs (implemented in next step)
    Ok(())
}
```

#### Step 3.2: Add Windows-Specific File Permissions
**File:** `src-tauri/Cargo.toml`
**Action:** Add Windows ACL dependency

```toml
[target.'cfg(target_os = "windows")'.dependencies]
winapi = { version = "0.3", features = ["winsock2", "aclapi", "accctrl", "securitybaseapi"] }
```

**File:** `src-tauri/src/api.rs`
**Action:** Implement Windows ACL security

```rust
#[cfg(target_os = "windows")]
fn set_secure_permissions_windows(path: &std::path::Path) -> std::io::Result<()> {
    use std::ptr;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::aclapi::SetNamedSecurityInfoW;
    use winapi::um::accctrl::{SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_W};
    use winapi::um::winnt::{DACL_SECURITY_INFORMATION, PSID, FILE_ALL_ACCESS, ACCESS_ALLOWED_ACE_TYPE};
    
    // Convert path to wide string
    let wide_path: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    
    // Get current user SID
    // Set ACL to only allow current user full access
    // This is a simplified version - full implementation would use SetNamedSecurityInfoW
    
    log::info!("Set Windows ACL on: {:?}", path);
    Ok(())
}
```

#### Step 3.3: Apply Permissions After Token Write
**File:** `src-tauri/src/api.rs`
**Action:** Modify token cache writing in `handle_callback`

```rust
// In handle_callback function, around line 229
if let Some(token) = spotify.get_token().lock().await.unwrap().clone() {
    log::debug!("Handle_callback: Attempting to cache token to: {:?}", spotify.config.cache_path);
    
    match token.write_cache(&spotify.config.cache_path) {
        Ok(_) => {
            log::debug!("Handle_callback: Successfully cached token");
            
            // NEW: Set secure permissions on token file
            if let Err(e) = set_secure_permissions(&spotify.config.cache_path) {
                log::warn!("Failed to set secure permissions on token cache: {}", e);
                // Continue - not critical enough to fail the auth flow
            }
        }
        Err(e) => log::error!("Handle_callback: Failed to cache token: {}", e),
    }
}
```

#### Step 3.4: Consider Token Encryption (Advanced)
For maximum security, encrypt tokens at rest:

**File:** `src-tauri/Cargo.toml`
```toml
[dependencies]
# Add encryption library
aes-gcm = "0.10"
rand = "0.8"
```

**Implementation:**
- Derive encryption key from system-specific data
- Encrypt token before writing
- Decrypt token after reading
- Store encryption nonce/salt separately

**Note:** This is an enhancement beyond the critical fix

#### Testing
- [ ] Token file created with correct permissions
- [ ] On Unix/Linux/macOS: Verify with `ls -l .spotify_token.json` (should show `-rw-------`)
- [ ] On Windows: Verify with `icacls .spotify_token.json` (should show only current user)
- [ ] Other user accounts cannot read token file
- [ ] Auth flow still works after permission changes
- [ ] Token refresh still works

**Estimated Time:** 4-6 hours  
**Dependencies:** None  
**Risk:** Medium - Need to test on all platforms

---

## 🔴 Issue #4: Unsafe Memory Operations (Windows)

### Current State
```rust
// src-tauri/src/api.rs:76-79
#[cfg(target_os = "windows")]
{
    let _wsa_data = unsafe {
        let mut data = std::mem::zeroed();
        WSAStartup(makeword(2, 2), &mut data);
        data
    };
}
```

### Risk Level
**MEDIUM** - Undefined behavior, resource leak, no error handling

### Problems
1. `std::mem::zeroed()` on `WSADATA` is potentially unsound
2. `WSAStartup` return value ignored (error checking missing)
3. `WSACleanup` never called (resource leak)
4. `_wsa_data` dropped immediately

### Implementation Plan

#### Step 4.1: Create Proper WSADATA Wrapper
**File:** `src-tauri/src/api.rs`
**Action:** Create RAII wrapper for WSA initialization

```rust
#[cfg(target_os = "windows")]
struct WsaGuard {
    _initialized: bool,
}

#[cfg(target_os = "windows")]
impl WsaGuard {
    fn new() -> Result<Self, std::io::Error> {
        use std::mem::MaybeUninit;
        use winapi::um::winsock2::WSAStartup;
        use winapi::shared::winerror::NO_ERROR;
        
        unsafe {
            let mut wsa_data = MaybeUninit::uninit();
            let result = WSAStartup(makeword(2, 2), wsa_data.as_mut_ptr());
            
            if result != NO_ERROR as i32 {
                return Err(std::io::Error::from_raw_os_error(result));
            }
            
            Ok(Self { _initialized: true })
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for WsaGuard {
    fn drop(&mut self) {
        use winapi::um::winsock2::WSACleanup;
        
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
```

#### Step 4.2: Use WsaGuard with Proper Lifetime
**File:** `src-tauri/src/api.rs`
**Action:** Modify `start_callback_server` to store WsaGuard

Option A: Store in static (requires lazy_static or once_cell)
```rust
#[cfg(target_os = "windows")]
use std::sync::OnceLock;

#[cfg(target_os = "windows")]
static WSA_GUARD: OnceLock<WsaGuard> = OnceLock::new();

fn start_callback_server(app_handle: AppHandle) {
    CALLBACK_SERVER.call_once(move || {
        #[cfg(target_os = "windows")]
        {
            match WsaGuard::new() {
                Ok(guard) => {
                    WSA_GUARD.set(guard).expect("WSA_GUARD already initialized");
                    log::info!("WSA initialized successfully");
                }
                Err(e) => {
                    log::error!("Failed to initialize WSA: {}", e);
                    return; // Don't start server if WSA fails
                }
            }
        }
        
        // Rest of callback server code...
```

Option B: Store with app state
```rust
// In main.rs, add to AppState
pub struct AppState {
    pub spotify: tokio::sync::Mutex<Option<AuthCodePkceSpotify>>,
    pub hotkey_hashmap: tokio::sync::Mutex<Option<HashMap<String, HotKey>>>,
    pub volume: tokio::sync::Mutex<u8>,
    #[cfg(target_os = "windows")]
    pub wsa_guard: Option<WsaGuard>,
}
```

#### Step 4.3: Handle Initialization Errors
**File:** `src-tauri/src/api.rs`
**Action:** Add error handling for WSA initialization failure

```rust
#[cfg(target_os = "windows")]
{
    if let Err(e) = WsaGuard::new() {
        log::error!("Failed to initialize Windows Sockets: {}", e);
        log::error!("Callback server will not start. OAuth may not work.");
        return; // Exit early if WSA initialization fails
    }
}
```

#### Step 4.4: Add Error Propagation
**File:** `src-tauri/src/api.rs`
**Action:** Make TcpListener::bind handle errors

```rust
// OLD:
let listener = TcpListener::bind("127.0.0.1:8888").unwrap();

// NEW:
let listener = match TcpListener::bind("127.0.0.1:8888") {
    Ok(l) => l,
    Err(e) => {
        log::error!("Failed to bind callback server to 127.0.0.1:8888: {}", e);
        log::error!("OAuth callback server will not work. Port may be in use.");
        return; // Exit thread gracefully
    }
};
```

#### Step 4.5: Add Unit Tests
**File:** `src-tauri/src/api.rs`
**Action:** Add tests for WSA handling

```rust
#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    
    #[test]
    fn test_wsa_guard_initialization() {
        let guard = WsaGuard::new();
        assert!(guard.is_ok(), "WSA initialization should succeed");
    }
    
    #[test]
    fn test_wsa_guard_cleanup() {
        {
            let _guard = WsaGuard::new().unwrap();
            // Guard should clean up when dropped
        }
        // Should be able to initialize again
        let guard2 = WsaGuard::new();
        assert!(guard2.is_ok(), "WSA should be re-initializable after cleanup");
    }
}
```

#### Testing
- [ ] Windows build compiles without warnings
- [ ] WSA initialization succeeds on Windows
- [ ] Callback server starts successfully
- [ ] OAuth flow works on Windows
- [ ] WSACleanup called on app exit (check with debugger)
- [ ] Proper error logged if WSA initialization fails
- [ ] TcpListener error handled gracefully if port in use
- [ ] No undefined behavior (run with Miri if possible)

**Estimated Time:** 3-4 hours  
**Dependencies:** None  
**Risk:** Low-Medium - Windows-specific, need Windows testing

---

## Implementation Timeline

### Week 1: Days 1-2
- [ ] **Issue #2: Implement CSP** (1-2 hours)
  - Easiest fix, immediate impact
  - Test thoroughly with all features

### Week 1: Days 2-3
- [ ] **Issue #1: Environment Variables** (2-3 hours)
  - Low risk, good practice
  - Update documentation

### Week 1: Days 3-5
- [ ] **Issue #3: Token Permissions** (4-6 hours)
  - Most complex, requires platform-specific code
  - Test on Windows, macOS, and Linux

### Week 1: Days 5-7
- [ ] **Issue #4: WSA Safety** (3-4 hours)
  - Windows-specific
  - Thorough testing required

### Week 1: Day 7
- [ ] **Final Testing & Integration**
  - Test all fixes together
  - Regression testing
  - Update security review document

**Total Estimated Time:** 12-16 hours

---

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_client_id_loaded() {
        assert!(!CLIENT_ID.is_empty(), "Client ID must be set");
    }
    
    #[test]
    #[cfg(unix)]
    fn test_token_permissions() {
        // Test secure file permissions
    }
    
    #[test]
    #[cfg(target_os = "windows")]
    fn test_wsa_initialization() {
        // Test WSA guard
    }
}
```

### Integration Tests
1. **Full OAuth Flow**
   - Login with Spotify
   - Verify token cached securely
   - Test token refresh

2. **Security Validation**
   - Verify CSP blocks unauthorized resources
   - Check file permissions on token cache
   - Verify no crashes from WSA handling

3. **Cross-Platform**
   - Test on Windows 10/11
   - Test on macOS (Intel & Apple Silicon)
   - Test on Linux (Ubuntu/Fedora)

### Manual Testing Checklist
- [ ] CSP doesn't break any features
- [ ] Build works with and without `SPOTIFY_CLIENT_ID` env var
- [ ] Token file has correct permissions on all platforms
- [ ] OAuth flow completes successfully
- [ ] App doesn't crash on startup (WSA)
- [ ] Callback server starts correctly
- [ ] No security warnings in logs

---

## Rollback Plan

If any critical fix causes issues:

1. **Issue #2 (CSP):**
   - Revert to `"csp": null` temporarily
   - Investigate CSP violations
   - Adjust policy and re-deploy

2. **Issue #1 (Environment Variable):**
   - Easy to rollback - just revert to hardcoded value
   - No runtime impact

3. **Issue #3 (File Permissions):**
   - Remove `set_secure_permissions()` call
   - File will be created with default permissions
   - No functional impact

4. **Issue #4 (WSA):**
   - Revert to unsafe block if issues arise
   - Add TODO comment for proper fix later

---

## Success Criteria

### Critical Fixes Complete When:
- ✅ CSP policy implemented and tested
- ✅ Client ID moved to environment variable
- ✅ Token cache has secure permissions on all platforms
- ✅ WSA initialization uses safe Rust patterns
- ✅ All tests pass
- ✅ No regressions in functionality
- ✅ Documentation updated

### Metrics:
- **Security Score:** Improve from MEDIUM to HIGH
- **Code Quality:** Eliminate all `unsafe` without proper justification
- **Test Coverage:** Add 15+ new tests
- **Documentation:** Add environment setup guide

---

## Resources Needed

### Development Environment
- Windows 10/11 machine for WSA testing
- macOS machine for Unix permissions testing
- Linux VM for additional Unix testing

### Tools
- `cargo audit` for dependency checking
- `cargo clippy` for lint checking
- CSP validator tools
- File permission checking utilities

### Documentation
- Spotify OAuth documentation
- Tauri security best practices
- Windows WSA API documentation
- Unix file permissions guide

---

## Post-Implementation

### After Fixes Are Deployed:
1. Update `SECURITY_REVIEW.md` with fix status
2. Create GitHub release notes highlighting security improvements
3. Notify users of security enhancements
4. Schedule follow-up security audit in 3 months
5. Consider bug bounty program for additional scrutiny

### Monitoring:
- Monitor for any CSP violations (if reporting enabled)
- Check logs for WSA initialization errors
- Monitor for auth flow failures
- Track file permission issues in bug reports

---

## Questions & Decisions Needed

### Before Starting:
1. **Q:** Should we rotate the Spotify Client ID after moving to env var?
   - **Decision:** [ ] Yes [ ] No [ ] Later

2. **Q:** Do we need token encryption or are file permissions sufficient?
   - **Decision:** [ ] Permissions only [ ] Add encryption [ ] Decide later

3. **Q:** Should we support multiple OAuth providers in future?
   - **Decision:** Affects architecture of Issue #1

4. **Q:** What's the backwards compatibility requirement?
   - **Decision:** Can we break existing token caches with permission changes?

---

## Notes

- All changes should be made in feature branches
- Create PR for each critical issue (or one combined PR)
- Request security-focused code review
- Consider external security audit after fixes
- Update changelog with security fix details

---

**Plan Status:** DRAFT  
**Last Updated:** 2026-02-11  
**Next Review:** After Week 1 implementation  
**Owner:** Development Team
