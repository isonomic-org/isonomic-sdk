// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The smallest Isonomic app. It lists the models its manifest lets it use,
//! then streams one reply from the first chat model, chunk by chunk.
//!
//! Everything it reaches goes through its [`AppContext`]: the client
//! scoped to its token, its settings, and its attention signal.
//!
//! Its sidebar hosts the window's shell header: the toggle, then the
//! header, in the top row; the header above the toggle when collapsed.

use isonomic_app::ui;

use isonomic_app::{App, AppContext, Attention, Manifest, ModelAccess, SearchHit, Sidebar};
use isonomic_proto::{Capabilities, Message, ModelInfo, StreamRequest};
use ui::{
    div, px, AnyView, AppContext as _, Context, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, WeakEntity, Window,
};

/// What the reply is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Loading,
    Streaming,
    Done,
    Failed(String),
}

pub struct Hello {
    app: AppContext,
    models: Vec<ModelInfo>,
    reply: String,
    phase: Phase,
    opened: Vec<String>,
}

/// The prompt Hello sends.
pub const PROMPT: &str = "Say hello";

impl Hello {
    pub fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    pub fn reply(&self) -> &str {
        &self.reply
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// Fold or open the sidebar, and tell the window.
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        let next = match self.app.sidebar() {
            Sidebar::Expanded => Sidebar::Collapsed,
            Sidebar::Collapsed => Sidebar::Expanded,
        };
        self.app.set_sidebar(next);
        cx.notify();
    }

    /// Deep links handed to it so far.
    pub fn opened(&self) -> &[String] {
        &self.opened
    }

    fn new(app: AppContext, cx: &mut Context<Self>) -> Self {
        let kept = app.clone();
        cx.spawn(async move |this, cx| {
            let result = run(&app, &this, cx).await;
            let _ = this.update(cx, |hello, cx| {
                hello.phase = match result {
                    Ok(()) => Phase::Done,
                    Err(message) => Phase::Failed(message),
                };
                cx.notify();
            });
            app.set_attention(Attention::Dot);
        })
        .detach();
        Self {
            app: kept,
            models: Vec::new(),
            reply: String::new(),
            phase: Phase::Loading,
            opened: Vec::new(),
        }
    }
}

/// List the models, then stream one reply into the view.
async fn run(
    app: &AppContext,
    this: &WeakEntity<Hello>,
    cx: &mut ui::AsyncApp,
) -> Result<(), String> {
    let models = app.client().models();
    let list = models.list().await.map_err(|e| e.to_string())?;
    let chat = list
        .iter()
        .find(|m| m.supports("chat"))
        .map(|m| m.id.clone());
    this.update(cx, |hello, cx| {
        hello.models = list;
        hello.phase = Phase::Streaming;
        cx.notify();
    })
    .map_err(|e| e.to_string())?;
    let Some(model) = chat else {
        return Err("no chat model available".into());
    };
    let request = StreamRequest {
        model: Some(model),
        messages: vec![Message::user(PROMPT)],
        ..Default::default()
    };
    let mut stream = models.stream(request).await.map_err(|e| e.to_string())?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        if let Some(error) = &chunk.error {
            return Err(error.message.clone());
        }
        this.update(cx, |hello, cx| {
            hello.reply.push_str(&chunk.text);
            cx.notify();
        })
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl App for Hello {
    fn manifest() -> Manifest {
        Manifest {
            id: "com.isonomic.hello".into(),
            name: "Hello".into(),
            version: "0.1.0".into(),
            icon: "sparkle".into(),
            capabilities: Capabilities {
                models: Some(ModelAccess {
                    any: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            schemas: vec![],
        }
    }

    fn build(cx: AppContext, _window: &mut Window, app: &mut ui::App) -> AnyView {
        app.new(|view| Hello::new(cx, view)).into()
    }

    fn hosts_shell_header() -> bool {
        true
    }

    fn open_link(&mut self, url: &str) -> bool {
        if !url.starts_with("isonomic://hello/") {
            return false;
        }
        self.opened.push(url.to_string());
        true
    }

    fn search(&mut self, q: &str) -> Vec<SearchHit> {
        let q = q.to_lowercase();
        self.models
            .iter()
            .filter(|m| m.name.to_lowercase().contains(&q))
            .map(|m| SearchHit {
                title: m.name.clone(),
                detail: Some("Model".into()),
                link: format!("isonomic://hello/models/{}", m.id),
            })
            .collect()
    }

    fn on_unload(&mut self) {
        self.reply.clear();
    }
}

impl Render for Hello {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let collapsed = self.app.sidebar() == Sidebar::Collapsed;
        let toggle = div()
            .id("sidebar-toggle")
            .child(if collapsed { ">" } else { "<" })
            .on_click(cx.listener(|hello, _, _, cx| hello.toggle_sidebar(cx)));
        let header = self.app.shell_header(window, cx);
        let top = div().pt(self.app.shell_inset());
        let top = if collapsed {
            top.flex().flex_col().child(header).child(toggle)
        } else {
            top.flex().flex_row().gap_2().child(toggle).child(header)
        };
        let sidebar = div().flex_none().child(top);
        let sidebar = if collapsed {
            sidebar.w(px(56.))
        } else {
            sidebar
        };
        let status = match &self.phase {
            Phase::Loading => "Loading models".to_string(),
            Phase::Streaming => "Answering".to_string(),
            Phase::Done => "Done".to_string(),
            Phase::Failed(message) => format!("Failed: {message}"),
        };
        let main = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().child(status))
            .children(self.models.iter().map(|m| div().child(m.name.clone())))
            .child(div().child(self.reply.clone()));
        div().flex().flex_row().child(sidebar).child(main)
    }
}
