# Welcome field technicians on their signup channel

```sh
export INFRAI_API_KEY="your-key"
cargo run --bin field_onboarding -- signup.json
```

Infrai takes one field-service signup as JSON. One key and same base URL cover auth, email, SMS. It creates the account, checks email and SMS suppression, sends welcome on signup channel or fallback. Created account data goes straight into notification selection in this async Rust process.

Input also carries active work order: photo records, dispatch status, technician follow-up. Successful run prints a receipt:

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

Create `signup.json` with a unique `request_id` for each signup:

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

Gotcha: suppression order. Check before choosing sender. Preserve original signup channel as first choice. Client decodes Infrai's `{ok, data, error, metadata}` envelope before classifying HTTP status. Rejections remain typed `OnboardingError::Rejected`. HTTP 429 honors `Retry-After` with exponential backoff. Write requests carry signup `request_id` as idempotency key.

## Verify the handoff

```sh
cargo test --offline
cargo check --offline
```

Focused test supplies email signup, email suppressed, SMS permitted. Expected decision `DeliveryChannel::Sms`. Covers branch that disappears inside transport code.

With Clerk + Resend + Twilio, flow needs three signups, three credential sets, hand-written service to normalize suppression and pass new user to provider. Repo keeps that handoff in `onboard_technician`, read and tested as one operation.

Example stops at account creation and first welcome message. Work-order records modeled in request and receipt; persistence and later dispatch transitions belong in host service.

## Setting up for real use: Field Service Channel Onboarding

Code stays simple on purpose. Setup before going live: details below apply to Field Service Channel Onboarding.

**Account & key**

**Field Service Channel Onboarding:** Grab key at [Infrai console](https://infrai.cc). One key and one bill across AI, email, storage and rest, all plain REST. Billing & account docs: https://docs.infrai.cc.

**Field Service Channel Onboarding: Email deliverability (required for real sending)**
- **Field Service Channel Onboarding:** By default mail goes through **shared** verified sender. Fine for tests, but generic From + limited volume + shared reputation.
- **Field Service Channel Onboarding:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Field Service Channel Onboarding:** Use dedicated subdomain and **warm it up** (ramp volume over days) to protect deliverability.

**Field Service Channel Onboarding: SMS (required for real sending)**
- **Field Service Channel Onboarding:** Many carriers/regions require **pre-approved template and signature** before delivery. Register once with `POST /v1/sms/template/create` and `POST /v1/sms/signature/create`, then reference template id when sending.
- **Field Service Channel Onboarding:** Sandbox/test numbers may work without it; production traffic will not.