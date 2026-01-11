# Authentication System

This document describes the authentication and authorization system.

## Overview

- **Sessions**: HTTP-only cookies, 7-day expiry
- **Password Storage**: Client SHA-256 → Server Argon2
- **Database**: Dual-database with per-user isolation

## Authentication Flow

```
┌──────────────────────────────────────────────────────────────────────┐
│                           CLIENT                                      │
│  ┌─────────────┐    SHA-256     ┌────────────────────────────────┐  │
│  │ password +  │───────────────►│ password_hash (64 hex chars)   │  │
│  │ username    │                └────────────────────────────────┘  │
│  └─────────────┘                              │                      │
└───────────────────────────────────────────────┼──────────────────────┘
                                                │ POST /login
                                                ▼
┌──────────────────────────────────────────────────────────────────────┐
│                           SERVER                                      │
│  ┌────────────────────────────────┐                                  │
│  │ password_hash from client      │                                  │
│  └────────────────────────────────┘                                  │
│                    │                                                  │
│                    │ Argon2                                          │
│                    ▼                                                  │
│  ┌────────────────────────────────┐     ┌──────────────────────┐    │
│  │ Argon2(password_hash)          │────►│ Stored in users.db   │    │
│  └────────────────────────────────┘     └──────────────────────┘    │
└──────────────────────────────────────────────────────────────────────┘
```

### Why Dual-Layer Hashing?

| Threat | Protection |
|--------|------------|
| Network sniffing (HTTPS compromised) | Client SHA-256 hides plaintext |
| Server database leak | Argon2 protects against rainbow tables |
| MITM attack | Attacker gets SHA-256, not plaintext |

Server never sees or stores plaintext passwords.

---

## Session Lifecycle

### Creation

```
Login Success
      │
      ▼
┌─────────────────────────────────────┐
│ Generate session ID (256-bit random)│
└─────────────────────────────────────┘
      │
      ▼
┌─────────────────────────────────────┐
│ INSERT INTO sessions:               │
│   id, user_id, created_at,          │
│   expires_at (+7 days), last_access │
└─────────────────────────────────────┘
      │
      ▼
┌─────────────────────────────────────┐
│ Set HTTP-only cookie: kr_session    │
│ Set readable cookie: kr_username    │
└─────────────────────────────────────┘
```

### Validation (Every Request)

```
Request with cookie
      │
      ▼
┌─────────────────────────────────────┐
│ Extract kr_session cookie           │
│ (Redirect to /login if missing)     │
└─────────────────────────────────────┘
      │
      ▼
┌─────────────────────────────────────┐
│ Query sessions table:               │
│   WHERE id = ? AND expires_at > now │
│ (Redirect to /login if invalid)     │
└─────────────────────────────────────┘
      │
      ▼
┌─────────────────────────────────────┐
│ Open user's learning.db             │
│ Run migrations if needed            │
│ ATTACH app.db for cross-DB queries  │
└─────────────────────────────────────┘
      │
      ▼
┌─────────────────────────────────────┐
│ Check admin status                  │
│ Check vocabulary access             │
│ Return AuthContext                  │
└─────────────────────────────────────┘
```

### Cookies

| Cookie | HttpOnly | Purpose |
|--------|----------|---------|
| `kr_session` | Yes | Session token (secure) |
| `kr_username` | No | Display name for navbar JS |

---

## Database Attachment

Each authenticated request attaches databases for cross-queries:

```
┌─────────────────────────────────────────────────────────────────┐
│ learning.db (user's)                                             │
│                                                                  │
│  ┌──────────────────┐     ┌─────────────────────────────────┐  │
│  │ card_progress    │────►│ ATTACH 'data/app.db' AS app     │  │
│  │ (card_id FK)     │     └─────────────────────────────────┘  │
│  └──────────────────┘                    │                      │
│                                          ▼                      │
│                            ┌─────────────────────────────────┐  │
│                            │ app.card_definitions            │  │
│                            │ (shared card content)           │  │
│                            └─────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

This allows queries like:
```sql
SELECT cp.*, cd.front, cd.main_answer
FROM card_progress cp
JOIN app.card_definitions cd ON cp.card_id = cd.id
```

---

## Admin Status

Admin is determined by two methods (backwards compatible):

```sql
SELECT CASE
    WHEN role = 'admin' THEN 1      -- Role-based (v4+)
    WHEN LOWER(username) = 'admin' THEN 1  -- Legacy username
    ELSE 0
END FROM users WHERE id = ?
```

### Admin Extractor

```rust
// Requires admin, returns 403 before form parsing
pub struct AdminContext { ... }

// Handler example
async fn admin_only(admin: AdminContext) -> impl IntoResponse {
    // Only admins reach here
}
```

---

## Permission Resolution

For pack access, permissions are checked in order:

```
┌──────────────────────────────────────┐
│ 1. Pack globally enabled?            │
│    (content_packs.is_enabled = 1)    │
│    ↓ No = Denied                     │
└──────────────────────────────────────┘
              │ Yes
              ▼
┌──────────────────────────────────────┐
│ 2. User is admin?                    │
│    ↓ Yes = Granted                   │
└──────────────────────────────────────┘
              │ No
              ▼
┌──────────────────────────────────────┐
│ 3. Pack has no permissions defined?  │
│    ↓ Yes = Admin-only (Denied)       │
└──────────────────────────────────────┘
              │ Has permissions
              ▼
┌──────────────────────────────────────┐
│ 4. Pack is public?                   │
│    (group_id = '' in pack_perms)     │
│    ↓ Yes = Granted                   │
└──────────────────────────────────────┘
              │ No
              ▼
┌──────────────────────────────────────┐
│ 5. User has direct permission?       │
│    (pack_user_permissions)           │
│    ↓ Yes = Granted                   │
└──────────────────────────────────────┘
              │ No
              ▼
┌──────────────────────────────────────┐
│ 6. User in allowed group?            │
│    (pack_permissions + membership)   │
│    ↓ Yes = Granted, No = Denied      │
└──────────────────────────────────────┘
```

---

## Extractors

| Extractor | Purpose | Rejection |
|-----------|---------|-----------|
| `AuthContext` | Require authentication | Redirect to `/login` |
| `AdminContext` | Require admin | 403 Forbidden |
| `OptionalAuth` | Optional auth | Never rejects |

### Usage

```rust
// Require login
async fn protected(auth: AuthContext) -> impl IntoResponse { ... }

// Require admin
async fn admin_only(admin: AdminContext) -> impl IntoResponse { ... }

// Optional (works logged in or out)
async fn public(OptionalAuth(auth): OptionalAuth) -> impl IntoResponse {
    match auth {
        Some(ctx) => format!("Hello, {}", ctx.username),
        None => "Hello, guest".to_string(),
    }
}
```

---

## Registration Flow

```
POST /register
      │
      ├─► Validate username (3-32 chars, alphanumeric + underscore)
      │
      ├─► Create user directory: data/users/{username}/
      │
      ├─► Initialize learning.db with schema
      │
      ├─► Seed baseline Hangul cards
      │
      ├─► Hash password (Argon2)
      │
      ├─► INSERT INTO users
      │
      └─► Auto-login (create session + set cookies)
```

---

## Guest Accounts

- Created via POST /guest
- Username: `guest_{random}`
- Password: auto-generated
- `is_guest = 1` flag in users table
- Expired via `last_activity_at` + `guest_expiry_hours` setting
- Admin can cleanup via `/settings/cleanup-guests`

---

## Source Files

| File | Purpose |
|------|---------|
| `src/auth/middleware.rs` | AuthContext, AdminContext extractors |
| `src/auth/handlers.rs` | Login, register, logout handlers |
| `src/auth/password.rs` | Argon2 hashing |
| `src/auth/db.rs` | User/session database operations |
| `src/session.rs` | Session ID generation |
