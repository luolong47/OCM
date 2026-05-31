//! Linux system tray integration for the backend process.

use std::process::Command;

use ksni::menu::StandardItem;
use ksni::{Category, Icon, MenuItem, Tray, TrayMethods};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy)]
pub enum TrayCommand {
    OpenPage,
    Exit,
}

pub type TrayHandle = ksni::Handle<OcmTray>;

pub struct OcmTray {
    tx: mpsc::UnboundedSender<TrayCommand>,
}

impl OcmTray {
    fn send(&self, command: TrayCommand) {
        let _ = self.tx.send(command);
    }
}

impl Tray for OcmTray {
    fn id(&self) -> String {
        "ocm-backend".into()
    }

    fn title(&self) -> String {
        "OCM".into()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn icon_name(&self) -> String {
        "applications-system".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![ocm_icon()]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayCommand::OpenPage);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "打开页面".into(),
                activate: Box::new(|tray: &mut Self| tray.send(TrayCommand::OpenPage)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "退出".into(),
                activate: Box::new(|tray: &mut Self| tray.send(TrayCommand::Exit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub async fn spawn(tx: mpsc::UnboundedSender<TrayCommand>) -> Result<TrayHandle, ksni::Error> {
    OcmTray { tx }.spawn().await
}

pub fn open_page(url: &str) {
    if let Err(err) = Command::new("xdg-open").arg(url).spawn() {
        tracing::warn!(%err, url, "failed to open frontend URL");
    }
}

fn ocm_icon() -> Icon {
    const SIZE: usize = 16;
    let mut data = Vec::with_capacity(SIZE * SIZE * 4);

    for y in 0..SIZE {
        for x in 0..SIZE {
            let border = x == 0 || y == 0 || x == SIZE - 1 || y == SIZE - 1;
            let diagonal = x == y || x + y == SIZE - 1;
            let (a, r, g, b) = if border {
                (255, 26, 88, 56)
            } else if diagonal {
                (255, 255, 255, 255)
            } else {
                (255, 42, 157, 93)
            };
            data.extend_from_slice(&[a, r, g, b]);
        }
    }

    Icon {
        width: SIZE as i32,
        height: SIZE as i32,
        data,
    }
}
