use std::rc::Rc;

use screen::Screen;

use super::motion::{Final, Motion};
use super::trigger::Trigger;

/// Names a container.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ContainerId(pub &'static str);

/// One way a container opens or closes: what triggers it and how it moves.
pub(crate) struct Entry {
    pub trigger: Box<dyn Trigger>,
    pub motion: Rc<dyn Motion>,
    /// `Open` for an open entry, `Closed` for a close entry.
    pub toward: Final,
}

/// A container's behaviour and children, built with [`container`].
pub struct ContainerSpec {
    pub(crate) id: ContainerId,
    pub(crate) screen: Box<dyn Screen>,
    pub(crate) entries: Vec<Entry>,
    pub(crate) children: Vec<ContainerSpec>,
    pub(crate) initially_open: bool,
}

pub fn container(id: ContainerId, screen: impl Screen) -> ContainerSpec {
    ContainerSpec {
        id,
        screen: Box::new(screen),
        entries: Vec::new(),
        children: Vec::new(),
        initially_open: false,
    }
}

impl ContainerSpec {
    fn entry(mut self, trigger: Box<dyn Trigger>, motion: Rc<dyn Motion>, toward: Final) -> Self {
        self.entries.push(Entry {
            trigger,
            motion,
            toward,
        });
        self
    }

    pub fn open(self, t: impl Trigger, m: impl Motion) -> Self {
        self.entry(Box::new(t), Rc::new(m), Final::Open)
    }

    pub fn close(self, t: impl Trigger, m: impl Motion) -> Self {
        self.entry(Box::new(t), Rc::new(m), Final::Closed)
    }

    /// Opens on `t` and closes on `t.mirrored()`, both with `m`.
    ///
    /// # Panics
    /// If `t` has no mirror.
    pub fn open_close(self, t: impl Trigger + Clone, m: impl Motion + Clone) -> Self {
        let mirror = t
            .mirrored()
            .expect("open_close needs a trigger with a mirror");
        self.entry(Box::new(t), Rc::new(m.clone()), Final::Open)
            .entry(mirror, Rc::new(m), Final::Closed)
    }

    pub fn child(mut self, c: ContainerSpec) -> Self {
        self.children.push(c);
        self
    }

    pub fn initially_open(mut self) -> Self {
        self.initially_open = true;
        self
    }
}
