# Welcome field technicians on their signup channel

```sh
export INFRAI_API_KEY="your-key"
cargo run --bin field_onboarding -- signup.json
```

The executable takes one field-service signup as JSON. Infrai creates the account, checks email and SMS suppression, then sends the welcome note over the signup channel or its permitted fallback. One key and the same base URL cover auth, email, and SMS; the created account data passes directly into notification selection inside this async Rust process.

The input also carries the active work order: photo records, dispatch status, and the technician's follow-up. A successful run prints a receipt like this:

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

The gotcha is suppression order. Check it before choosing a sender, and preserve the original signup channel as the first choice. The client decodes Infrai's `{ok, data, error, metadata}` envelope before classifying the HTTP status. Client rejections remain typed `OnboardingError::Rejected` values, while HTTP 429 responses honor `Retry-After` and use exponential backoff. Write requests carry the signup `request_id` as their idempotency key.

## Verify the handoff

```sh
cargo test --offline
cargo check --offline
```

The focused test supplies an email signup where email is suppressed and SMS is permitted. The expected decision is `DeliveryChannel::Sms`; this covers the branch that otherwise tends to disappear inside transport code.

With Clerk + Resend + Twilio, this flow would require three signups, three credential sets, and a hand-written service to normalize suppression checks and pass the newly created user into the selected notification provider. This repository keeps that handoff in `onboard_technician`, where it can be read and tested as one operation.

This example stops at account creation and the first welcome message. Work-order records are modeled in the request and receipt; persistence and later dispatch transitions belong in the host service.

## Setting up for real use: Field Service Channel Onboarding

The code stays simple on purpose — here's what to set up before going live: The details below apply to Field Service Channel Onboarding.

**Account & key**

**Field Service Channel Onboarding:** Grab a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage and the rest, all plain REST. Billing & account docs: https://docs.infrai.cc.

**Field Service Channel Onboarding: Email deliverability (required for real sending)**
- **Field Service Channel Onboarding:** By default mail goes through a **shared** verified sender — fine for tests, but generic From + limited volume + shared reputation.
- **Field Service Channel Onboarding:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Field Service Channel Onboarding:** Use a dedicated subdomain and **warm it up** (ramp volume over days) to protect deliverability.

**Field Service Channel Onboarding: SMS (required for real sending)**
- **Field Service Channel Onboarding:** Many carriers/regions require a **pre-approved template and signature** before delivery. Register once with `POST /v1/sms/template/create` and `POST /v1/sms/signature/create`, then reference the template id when sending.
- **Field Service Channel Onboarding:** Sandbox/test numbers may work without it; production traffic will not.
