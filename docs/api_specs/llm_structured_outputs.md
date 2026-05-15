# LLM Structured Output Spec

Source references:
- OpenAI Structured Outputs: https://developers.openai.com/api/docs/guides/structured-outputs
- Anthropic Structured Outputs: https://platform.claude.com/docs/en/build-with-claude/structured-outputs
- Gemini Structured Output: https://ai.google.dev/gemini-api/docs/structured-output

## Product decision

All LLM providers are user-key based. The system must support multiple providers, but internally normalize every result into the same `ScenarioOutput` contract.

## Providers

```text
openai
anthropic
gemini
local_later
```

## Core rule

LLM may produce:

```text
- analysis text
- scenario probabilities
- recommended action intent
- evidence_refs
```

LLM must not produce final broker order. Final order quantity and broker call are controlled by Risk Engine + Execution Engine.

## Common internal trait

```rust
#[async_trait::async_trait]
pub trait LlmProvider {
    async fn generate_scenario(
        &self,
        input: ScenarioPromptInput,
        output_schema: serde_json::Value,
    ) -> anyhow::Result<ScenarioOutput>;
}
```

## OpenAI adapter

Use Structured Outputs / JSON Schema. Adapter should pass the JSON schema from `contracts/scenario_output.schema.json` and reject nonconforming responses.

Recommended call behavior:

```text
- temperature: 0.2-0.4
- max output tokens: configurable
- response format: JSON schema
- retries: 1 schema retry maximum
```

## Anthropic adapter

Use Claude JSON schema/structured output support when available. If provider behavior changes, fallback adapter may use tool-use schema forcing. Output is still parsed into `ScenarioOutput`.

## Gemini adapter

Use response schema / structured output. Gemini supports a subset of JSON Schema, so keep scenario schema simple:

```text
string, number, integer, boolean, object, array, enum
```

Avoid advanced JSON Schema features in the cross-provider schema.

## Cross-provider validation

Always perform local validation after model response:

```rust
jsonschema::validator_for(&schema)?.validate(&value)?;
```

Then business validation:

```text
- probabilities sum roughly 100; allow tolerance 5
- scenario types include bullish/sideways/bearish
- recommended_action action is one of buy/sell/hold/watch
- evidence_refs exist in input evidence cards
- target/stop prices are positive decimals
- if action is buy/sell, condition text exists
```

## Cost control

Per manager:

```text
- daily LLM call limit
- daily cost limit
- max symbols per run
- max evidence cards per symbol
- max schedule slots per day warning
```

The UI should show a warning when users configure many 5-minute scenario slots.
