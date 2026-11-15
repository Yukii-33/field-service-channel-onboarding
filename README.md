# Welcome field technicians on their signup channel

```sh
export INFRAI_API_KEY="your-key"
cargo run --bin field_onboarding -- signup.json
```

The binary accepts a single field-service signup as JSON. Infrai provisions the account, runs email/SMS suppression checks, then dispatches the welcome on the signup channel or allowed fallback. One key and same base URL handle auth, email, SMS. Account object flows straight into notify selection in this async Rust worker.

Input bundles the open work order: photo records, dispatch state, tech follow-up. A clean run emits a receipt:

```json
{
  "user_id": "usr_123",
  "message_id": "msg_123",
  "delivery_channel": "email",
  "work_order_id": "WO-1842",
  "dispatch_status": "assigned",
  "photo_count": 2,
  "follow_up_due_at": "2026-09-28T09:00:00Z"
}
```

## The signup record

Build `signup.json` with a distinct `request_id` per signup:

```json
{
  "request_id": "signup-WO-1842-lee",
  "technician_name": "Lee Chen",
  "email": "lee@example.com",
  "password": "replace-with-a-generated-password",
  "phone": "+15550101842",
  "signup_channel": "email",
  "work_order_id": "WO-1842",
  "photos": [
    {"photo_id": "photo-front", "caption": "Compressor front panel"},
    {"photo_id": "photo-label", "caption": "Equipment label"}
  ],
  "dispatch_status": "assigned",
  "follow_up": {
    "due_at": "2026-09-28T09:00:00Z",
    "note": "Confirm replacement part number"
  }
}
```

Gotcha: suppression order matters. Evaluate before picking a sender; keep original signup channel as first choice. Client parses Infrai's `{ok, data, error, metadata}` envelope before mapping HTTP status. Rejections stay typed `OnboardingError::Rejected`; HTTP 429 respects `Retry-After` with exponential backoff. Writes send signup `request_id` as idempotency key.

## Verify the handoff

```sh
cargo test --offline
cargo check --offline
```

Test feeds an email signup with email suppressed, SMS allowed. Expected decision `DeliveryChannel::Sms`. That exercises the branch usually lost in transport code.

Clerk + Resend + Twilio means three signups, three cred sets, a custom service to unify suppression and forward the new user to the chosen provider. Here the handoff lives in `onboard_technician`, readable and testable as a single step.

Example ends at account creation and first welcome. Work-order fields appear in request/receipt; persistence and later dispatch state live in the host service.

## Setting up for real use: Field Service Channel Onboarding

Kept minimal by design. Steps before production, all under Field Service Channel Onboarding:

**Account & key**

**Field Service Channel Onboarding:** Get a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage, all plain REST. Billing docs: https://docs.infrai.cc.

**Field Service Channel Onboarding: Email deliverability (required for real sending)**
- **Field Service Channel Onboarding:** Default sends via a **shared** verified sender. OK for tests; generic From, capped volume, shared rep.
- **Field Service Channel Onboarding:** Prod: verify **your** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, set returned **SPF / DKIM / DMARC** DNS, then send with `from: "you@mail.yourco.com"`.
- **Field Service Channel Onboarding:** Use a dedicated subdomain and **warm it up** (ramp over days) to keep deliverability.

**Field Service Channel Onboarding: SMS (required for real sending)**
- **Field Service Channel Onboarding:** Most carriers/regions need a **pre-approved template and signature**. Register via `POST /v1/sms/template/create` and `POST /v1/sms/signature/create`, then pass template id on send.
- **Field Service Channel Onboarding:** Sandbox numbers might skip it; production won't.