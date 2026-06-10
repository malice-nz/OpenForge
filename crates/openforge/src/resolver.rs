use crate::models::ModFile;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone)]
pub struct ResolvedNode {
    pub project_id: u64,
    pub file_id: u64,
    pub required: bool,
}

pub struct DependencyResolver;

impl DependencyResolver {
    pub fn collect_required(root: &ModFile) -> Vec<u64> {
        root.dependencies
            .iter()
            .filter(|d| d.relation_type == 3)
            .map(|d| d.mod_id)
            .collect()
    }

    pub fn walk<F: FnMut(u64) -> Option<ModFile>>(roots: &[u64], mut fetch_latest: F) -> Vec<ResolvedNode> {
        let mut seen: HashSet<u64> = HashSet::new();
        let mut queue: VecDeque<(u64, bool)> = roots.iter().map(|id| (*id, true)).collect();
        let mut out = Vec::new();
        while let Some((pid, required)) = queue.pop_front() {
            if !seen.insert(pid) { continue; }
            let Some(file) = fetch_latest(pid) else { continue; };
            out.push(ResolvedNode { project_id: pid, file_id: file.id, required });
            for dep in &file.dependencies {
                let is_required = dep.relation_type == 3;
                let is_optional = dep.relation_type == 2;
                if is_required || is_optional {
                    queue.push_back((dep.mod_id, is_required));
                }
            }
        }
        out
    }
}

pub fn group_by_project(files: &[ModFile]) -> HashMap<u64, Vec<&ModFile>> {
    let mut m: HashMap<u64, Vec<&ModFile>> = HashMap::new();
    for f in files { m.entry(f.project_id).or_default().push(f); }
    m
}
