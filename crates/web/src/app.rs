use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{ParentRoute, Route, Router, Routes},
    path,
};

use crate::pages::{
    home::HomePage,
    manager::analysis::AnalysisTab,
    manager::detail::ManagerDetailPage,
    manager::holdings::HoldingsTab,
    manager::scenarios::ScenariosTab,
    manager::schedule::ScheduleTab,
    manager::settings::SettingsTab,
    manager::trades::TradesTab,
    manager_new::ManagerNewPage,
    managers::ManagersPage,
    not_found::NotFoundPage,
    settings::SettingsPage,
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="ko">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <link rel="manifest" href="/manifest.json"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="lumos" href="/pkg/lumos.css"/>
        <Title text="Lumos — AI Fund Manager"/>

        <Router>
            <Routes fallback=|| view! { <NotFoundPage/> }>
                <Route path=path!("/") view=HomePage/>
                <Route path=path!("/managers") view=ManagersPage/>
                <Route path=path!("/managers/new") view=ManagerNewPage/>
                <Route path=path!("/settings") view=SettingsPage/>
                <ParentRoute path=path!("/managers/:id") view=ManagerDetailPage>
                    <Route path=path!("") view=ScenariosTab/>
                    <Route path=path!("/holdings") view=HoldingsTab/>
                    <Route path=path!("/trades") view=TradesTab/>
                    <Route path=path!("/schedule") view=ScheduleTab/>
                    <Route path=path!("/settings") view=SettingsTab/>
                    <Route path=path!("/analysis") view=AnalysisTab/>
                </ParentRoute>
            </Routes>
        </Router>
    }
}
