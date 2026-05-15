# Risk Gate Prompt v1

이 문서는 LLM 프롬프트가 아니라 애플리케이션 코드로 강제해야 하는 리스크 검증 규칙입니다. LLM은 주문 의도를 만들 수 있지만, Risk Engine만 주문 계획을 승인할 수 있습니다.

## 기본값

- 주문 방식: 지정가만 허용
- KRX: 정규장만 허용
- 미국: 정규장만 허용
- 종목당 최대 비중: 5%
- 1회 최대 주문금액: 1,000,000 KRW 상당액
- 일일 손실률 -2% 도달 시 해당 매니저 자동매매 정지
- 매니저별 auto_trade_enabled 기본값: false
- 실전 주문은 manager.mode = live, broker_connection.environment = real인 경우에만 허용
- 모의 주문은 manager.mode = paper, PaperBroker에서만 허용

## 주문 전 필수 검증

1. 매니저 상태가 active인가?
2. auto_trade_enabled가 true인가?
3. 전체 시스템 kill switch가 꺼져 있는가?
4. 현재 시간이 허용된 정규장인가?
5. 해당 시장이 휴장일이 아닌가?
6. 가격 데이터가 fresh인가?
7. 계좌/잔고 데이터가 fresh인가?
8. 주문 가능 금액이 충분한가?
9. 주문 방식이 limit인가?
10. 주문 금액이 1회 최대 주문금액 이하인가?
11. 매수 후 종목 비중이 5% 이하인가?
12. 당일 손실률이 -2%를 넘지 않았는가?
13. 당일 매매 횟수 제한을 넘지 않았는가?
14. 같은 idempotency_key의 주문이 이미 존재하지 않는가?
15. 공시/뉴스/데이터 수집 실패가 blocking 상태가 아닌가?

## 거절 사유 예시

- `market_closed`: 정규장 아님
- `holiday`: 휴장일
- `stale_price`: 가격 데이터 오래됨
- `stale_account`: 계좌 데이터 오래됨
- `order_type_blocked`: 지정가가 아님
- `max_order_amount_exceeded`: 1회 주문금액 초과
- `max_position_pct_exceeded`: 종목 비중 5% 초과
- `daily_loss_limit_exceeded`: 일일 손실률 -2% 초과
- `duplicate_order`: 중복 주문
- `broker_not_verified`: 브로커 연결 미검증
- `data_blocking`: 핵심 데이터 수집 실패

## 주문 수량 계산 원칙

- AI가 수량을 확정하지 않습니다.
- Risk Engine이 현금, 주문 가능 금액, 최대 비중, 현재 보유량을 기준으로 수량을 계산합니다.
- 미국 주식 소수점 거래는 MVP에서는 비활성화하고 정수 수량으로 시작합니다.
- 매도 수량은 보유 수량을 초과할 수 없습니다.
