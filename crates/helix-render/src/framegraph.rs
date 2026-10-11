//! Declared pass graph (FG-1..3 mini): passes + resources as data, validated
//! at boot, executed in declaration order. Imports may be written by several
//! passes; transients may not (live first-write .. last-read).
#[derive(Clone, Copy)]
pub enum SizePolicy {
    MatchSwapchain,
    Absolute(u32, u32),
}

#[derive(Clone, Copy)]
pub enum ResourceKind {
    Import,
    Transient(SizePolicy),
}

pub struct Resource {
    pub name: &'static str,
    pub kind: ResourceKind,
}

pub struct Pass {
    pub id: &'static str,
    pub reads: &'static [&'static str],
    pub writes: &'static [&'static str],
}

pub struct FrameGraph {
    resources: Vec<Resource>,
    passes: Vec<Pass>,
}

impl FrameGraph {
    /// Declaration order IS execution order (FG-1: predictable > clever).
    pub fn default_graph() -> Self {
        Self {
            resources: vec![
                Resource { name: "swapchain", kind: ResourceKind::Import },
                Resource {
                    name: "hdr_color",
                    kind: ResourceKind::Transient(SizePolicy::MatchSwapchain),
                },
                Resource {
                    name: "depth",
                    kind: ResourceKind::Transient(SizePolicy::MatchSwapchain),
                },
            ],
            passes: vec![
                Pass { id: "MainPass", reads: &[], writes: &["hdr_color", "depth"] },
                Pass { id: "PostPass", reads: &["hdr_color"], writes: &["swapchain"] },
            ],
        }
    }

    /// Structural soundness is fatal; unused transients warn (boot log).
    pub fn validate(&self) -> Result<Vec<String>, String> {
        let mut warnings = Vec::new();
        let imported: Vec<&str> = self
            .resources
            .iter()
            .filter(|r| matches!(r.kind, ResourceKind::Import))
            .map(|r| r.name)
            .collect();
        let mut produced = imported.clone();
        let (mut ever_written, mut read) = (Vec::new(), Vec::new());
        for p in &self.passes {
            for &wr in p.writes {
                let is_import = imported.contains(&wr);
                if produced.contains(&wr) && !is_import {
                    return Err(format!("pass '{}' double-writes transient '{}'", p.id, wr));
                }
                produced.push(wr);
                ever_written.push(wr);
            }
            for &rd in p.reads {
                if !produced.contains(&rd) {
                    return Err(format!(
                        "pass '{}' reads '{}' before any producer (declaration = execution order)",
                        p.id, rd
                    ));
                }
                read.push(rd);
            }
        }
        for r in &self.resources {
            if let ResourceKind::Transient(_) = r.kind {
                if !ever_written.contains(&r.name) {
                    warnings.push(format!("transient '{}' never written", r.name));
                } else if !read.contains(&r.name) {
                    warnings.push(format!("transient '{}' written but never read", r.name));
                }
            }
        }
        Ok(warnings)
    }

    /// FG-6: dev-visible graph dump.
    pub fn dump_dot(&self) -> String {
        let mut s = String::from("digraph framegraph {\n  rankdir=LR;\n");
        for r in &self.resources {
            let k = match r.kind {
                ResourceKind::Import => "import",
                _ => "transient",
            };
            s += &format!("  \"{}\"[shape=box, label=\"{}\\n({})\"];\n", r.name, r.name, k);
        }
        for p in &self.passes {
            s += &format!("  \"{}\"[shape=ellipse];\n", p.id);
            for &rd in p.reads {
                s += &format!("  \"{rd}\" -> \"{}\";\n", p.id);
            }
            for &wr in p.writes {
                s += &format!("  \"{}\" -> \"{wr}\";\n", p.id);
            }
        }
        s += "}\n";
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [T] FG-1/2 — boot validation: order, dead transients warn, errors fatal.
    #[test]
    fn validation() {
        let g = FrameGraph::default_graph();
        assert!(g.validate().unwrap().is_empty());

        let bad = FrameGraph {
            resources: vec![Resource {
                name: "a",
                kind: ResourceKind::Transient(SizePolicy::MatchSwapchain),
            }],
            passes: vec![Pass { id: "P", reads: &["a"], writes: &[] }],
        };
        assert!(bad.validate().is_err());
    }
}
