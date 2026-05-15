# Telegram Bot API Spec

Source reference:
- https://core.telegram.org/bots/api

## Role in this product

Telegram is the first notification channel for automatic trading events.

## Auth

Telegram Bot API uses bot token in URL:

```text
https://api.telegram.org/bot{BOT_TOKEN}/{METHOD}
```

Store user bot token encrypted.

## Methods needed

### getMe

```text
GET or POST /bot{token}/getMe
```

Use to verify token.

### sendMessage

```text
POST /bot{token}/sendMessage
Content-Type: application/json
```

Body:

```json
{
  "chat_id": "123456789",
  "text": "message",
  "parse_mode": "HTML",
  "disable_web_page_preview": true
}
```

Required:

```text
chat_id
text
```

## Notification events

```text
scenario_generated
risk_rejected
order_plan_created
order_submitted
order_filled
order_partially_filled
order_rejected
order_canceled
daily_loss_limit_reached
api_key_error
data_collection_error
manager_paused
```

## Message templates

### Order submitted

```text
[매니저명] 주문 접수
종목: 삼성전자 (005930)
방향: BUY
수량: 10주
지정가: 70,000원
근거: 저항선 돌파 + 거래량 증가
리스크: 종목비중 4.8%, 일일손실 정상
```

### Risk rejected

```text
[매니저명] 주문 차단
종목: AAPL
방향: BUY
사유: 종목 비중 5% 초과
시나리오: 상승 52%, 목표가 195.00
```

## DB

```sql
CREATE TABLE notification_channels (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    channel_type TEXT NOT NULL CHECK (channel_type IN ('telegram')),
    enabled BOOLEAN NOT NULL DEFAULT true,
    encrypted_config_json BYTEA NOT NULL,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE notification_events (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    manager_id UUID REFERENCES managers(id),
    channel_id UUID REFERENCES notification_channels(id),
    event_type TEXT NOT NULL,
    title TEXT NOT NULL,
    message TEXT NOT NULL,
    payload_json JSONB,
    status TEXT NOT NULL CHECK (status IN ('pending','sent','failed')),
    sent_at TIMESTAMPTZ,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

## UX flow

```text
1. User creates Telegram bot via BotFather.
2. User inputs bot token in app.
3. App calls getMe to verify.
4. User sends a message to the bot.
5. App reads chat_id via getUpdates, or user manually inputs chat_id.
6. App sends test message.
7. Mark channel verified.
```

For privacy and simplicity in MVP, manual chat_id input is acceptable.
