use leptos::prelude::*;

use crate::api::client::{create_llm_key, delete_llm_key, fetch_llm_keys, LlmKeyDto};
use crate::components::layout::{AppLayout, PageHeader};

#[component]
pub fn LlmKeysPage() -> impl IntoView {
    let show_form = RwSignal::new(false);
    let refresh_trigger = RwSignal::new(0);

    let keys = LocalResource::new(move || {
        refresh_trigger.get();
        async move { fetch_llm_keys().await.ok() }
    });

    let refresh = move || {
        refresh_trigger.update(|n| *n += 1);
    };

    view! {
        <AppLayout>
            <PageHeader title="LLM API 키 관리"/>

            <div style="max-width:800px;">
                <div style="margin-bottom:24px;">
                    <button
                        class="btn btn-primary btn-sm"
                        on:click=move |_| show_form.set(!show_form.get())
                    >
                        {move || if show_form.get() { "취소" } else { "+ 새 키 등록" }}
                    </button>
                </div>

                {move || show_form.get().then(|| view! {
                    <AddKeyForm on_success=refresh/>
                })}

                <Suspense fallback=|| view! { <p class="text-muted">"로딩 중..."</p> }>
                    {move || match keys.get().map(|w| (*w).clone()) {
                        None => view! { <p class="text-muted">"로딩 중..."</p> }.into_any(),
                        Some(None) => view! {
                            <div class="alert alert-error">"키 목록 로딩 실패"</div>
                        }.into_any(),
                        Some(Some(list)) if list.is_empty() => view! {
                            <div class="card">
                                <p class="text-muted">"등록된 LLM API 키가 없습니다."</p>
                            </div>
                        }.into_any(),
                        Some(Some(list)) => view! {
                            <div style="display:flex;flex-direction:column;gap:16px;">
                                {list.into_iter().map(|key| {
                                    view! { <KeyCard key=key on_delete=refresh/> }
                                }).collect_view()}
                            </div>
                        }.into_any(),
                    }}
                </Suspense>
            </div>
        </AppLayout>
    }
}

#[component]
fn AddKeyForm(on_success: impl Fn() + 'static + Copy) -> impl IntoView {
    let provider = RwSignal::new("openai".to_string());
    let label = RwSignal::new(String::new());
    let api_key = RwSignal::new(String::new());
    let submitting = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();

        let provider_val = provider.get();
        let label_val = label.get();
        let api_key_val = api_key.get();

        submitting.set(true);
        error.set(None);

        leptos::task::spawn_local(async move {
            match create_llm_key(&provider_val, &label_val, &api_key_val).await {
                Ok(_) => {
                    label.set(String::new());
                    api_key.set(String::new());
                    on_success();
                }
                Err(e) => {
                    error.set(Some(format!("등록 실패: {}", e)));
                }
            }
            submitting.set(false);
        });
    };

    view! {
        <div class="card" style="margin-bottom:24px;">
            <div class="card-section-title">"새 LLM API 키 등록"</div>

            {move || error.get().map(|e| view! {
                <div class="alert alert-error" style="margin-bottom:16px;">{e}</div>
            })}

            <form on:submit=on_submit>
                <div class="form-group">
                    <label class="form-label">"프로바이더"</label>
                    <select
                        class="form-input"
                        on:change=move |ev| provider.set(event_target_value(&ev))
                    >
                        <option value="openai" selected>{move || if provider.get() == "openai" { "selected" } else { "" }} "OpenAI"</option>
                        <option value="gemini">{move || if provider.get() == "gemini" { "selected" } else { "" }} "Google Gemini"</option>
                    </select>
                </div>

                <div class="form-group">
                    <label class="form-label">"레이블"</label>
                    <input
                        type="text"
                        class="form-input"
                        placeholder="예: My OpenAI Key"
                        on:input=move |ev| label.set(event_target_value(&ev))
                        required
                    />
                </div>

                <div class="form-group">
                    <label class="form-label">"API 키"</label>
                    <input
                        type="password"
                        class="form-input"
                        placeholder="API 키 입력"
                        on:input=move |ev| api_key.set(event_target_value(&ev))
                        required
                    />
                    <div class="form-hint">
                        {move || {
                            if provider.get() == "gemini" {
                                "Google AI Studio에서 발급: https://ai.google.dev/"
                            } else {
                                "OpenAI 대시보드에서 발급: https://platform.openai.com/api-keys"
                            }
                        }}
                    </div>
                </div>

                <button
                    type="submit"
                    class="btn btn-primary btn-sm"
                    prop:disabled=move || submitting.get()
                >
                    {move || if submitting.get() { "등록 중..." } else { "등록" }}
                </button>
            </form>
        </div>
    }
}

#[component]
fn KeyCard(key: LlmKeyDto, on_delete: impl Fn() + 'static + Copy) -> impl IntoView {
    let deleting = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let key_id = key.id;

    let on_delete_click = move |_| {
        deleting.set(true);
        error.set(None);

        leptos::task::spawn_local(async move {
            match delete_llm_key(key_id).await {
                Ok(_) => on_delete(),
                Err(e) => error.set(Some(format!("삭제 실패: {}", e))),
            }
            deleting.set(false);
        });
    };

    let provider_badge_class = if key.provider == "gemini" {
        "badge-blue"
    } else {
        "badge-green"
    };

    view! {
        <div class="card">
            {move || error.get().map(|e| view! {
                <div class="alert alert-error" style="margin-bottom:12px;">{e}</div>
            })}

            <div style="display:flex;align-items:start;justify-content:space-between;gap:16px;">
                <div style="flex:1;">
                    <div style="display:flex;align-items:center;gap:8px;margin-bottom:8px;">
                        <span class=format!("badge {}", provider_badge_class)>
                            {key.provider.to_uppercase()}
                        </span>
                        <span class="font-semibold">{key.label.clone()}</span>
                    </div>

                    <div style="display:flex;flex-direction:column;gap:4px;">
                        <div class="text-muted" style="font-size:12px;">
                            "키: " <code>{key.masked_hint.unwrap_or_else(|| "***".to_string())}</code>
                        </div>
                        <div class="text-muted" style="font-size:12px;">
                            "등록일: " {key.created_at.clone()}
                        </div>
                    </div>
                </div>

                <button
                    class="btn btn-ghost btn-sm"
                    style="color:var(--color-danger);"
                    on:click=on_delete_click
                    prop:disabled=move || deleting.get()
                >
                    {move || if deleting.get() { "삭제 중..." } else { "삭제" }}
                </button>
            </div>
        </div>
    }
}
