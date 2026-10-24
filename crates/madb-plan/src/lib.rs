//! Deterministic task planning. No IO, backend or wire-format dependencies.
use madb_model::{Id, LogicalPath, ValueError};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Metadata,
    Generate,
    Compile,
    Link,
    Run,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task {
    pub id: Id,
    pub phase: Phase,
    pub dependencies: Vec<Id>,
    pub outputs: Vec<LogicalPath>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskGraph {
    tasks: Vec<Task>,
}

impl TaskGraph {
    pub fn new(tasks: Vec<Task>) -> Result<Self, ValueError> {
        if tasks.is_empty() || tasks.len() > 4096 {
            return Err(ValueError("task graph size out of range"));
        }
        let mut by_id = BTreeMap::new();
        for task in tasks {
            if by_id.insert(task.id.clone(), task).is_some() {
                return Err(ValueError("duplicate task identity"));
            }
        }
        for task in by_id.values() {
            let mut seen = BTreeSet::new();
            for dependency in &task.dependencies {
                if !by_id.contains_key(dependency) {
                    return Err(ValueError("missing task dependency"));
                }
                if !seen.insert(dependency) {
                    return Err(ValueError("duplicate task dependency"));
                }
            }
        }
        // Track every prefix, not only complete files: Out/a and out/b
        // alias the same directory on common consumer filesystems.
        let mut outputs = BTreeMap::<String, (String, bool)>::new();
        for task in by_id.values() {
            for logical in &task.outputs {
                let path = logical.as_str();
                if !path.is_ascii() {
                    return Err(ValueError("portable task outputs require ASCII paths"));
                }
                for end in path.match_indices('/').map(|(i, _)| i).chain([path.len()]) {
                    let prefix = &path[..end];
                    let is_file = end == path.len();
                    match outputs.entry(prefix.to_ascii_lowercase()) {
                        std::collections::btree_map::Entry::Occupied(previous) => {
                            let (spelling, was_file) = previous.get();
                            if spelling != prefix {
                                return Err(ValueError("case-aliased task output prefix"));
                            }
                            if *was_file || is_file {
                                return Err(ValueError(
                                    "duplicate task output or file/directory collision",
                                ));
                            }
                        }
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert((prefix.to_owned(), is_file));
                        }
                    }
                }
            }
        }
        let mut sorted = Vec::new();
        let mut complete = BTreeSet::new();
        while sorted.len() < by_id.len() {
            let ready = by_id.values().find(|task| {
                !complete.contains(&task.id)
                    && task.dependencies.iter().all(|id| complete.contains(id))
            });
            let task = ready.ok_or(ValueError("task dependency cycle"))?;
            complete.insert(task.id.clone());
            sorted.push(task.clone());
        }
        Ok(Self { tasks: sorted })
    }
    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }
    pub fn reject_active(&self, active: &[Id]) -> Result<(), ValueError> {
        if self.tasks.iter().any(|t| active.contains(&t.id)) {
            return Err(ValueError("recursive application task invocation"));
        }
        Ok(())
    }
    pub fn subgraph(&self, target: &Id, active: &[Id]) -> Result<Self, ValueError> {
        let mut wanted = BTreeSet::new();
        let mut pending = vec![target.clone()];
        while let Some(id) = pending.pop() {
            if !wanted.insert(id.clone()) {
                continue;
            }
            if active.contains(&id) {
                return Err(ValueError("recursive application task invocation"));
            }
            let task = self
                .tasks
                .iter()
                .find(|task| task.id == id)
                .ok_or(ValueError("unknown named task"))?;
            pending.extend(task.dependencies.iter().cloned());
        }
        Self::new(
            self.tasks
                .iter()
                .filter(|task| wanted.contains(&task.id))
                .cloned()
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn task(id: &str, phase: Phase, dependencies: &[&str]) -> Task {
        Task {
            id: Id::new(id).unwrap(),
            phase,
            dependencies: dependencies.iter().map(|x| Id::new(*x).unwrap()).collect(),
            outputs: vec![],
        }
    }
    #[test]
    fn deterministic_and_named_subgraphs() {
        let a = task("a", Phase::Metadata, &[]);
        let b = task("b", Phase::Generate, &["a"]);
        let c = task("c", Phase::Compile, &["b"]);
        assert_eq!(
            TaskGraph::new(vec![c.clone(), b.clone(), a.clone()]),
            TaskGraph::new(vec![a, b.clone(), c])
        );
        let graph = TaskGraph::new(vec![task("a", Phase::Metadata, &[]), b]).unwrap();
        assert_eq!(
            graph
                .subgraph(&Id::new("a").unwrap(), &[])
                .unwrap()
                .tasks()
                .len(),
            1
        );
        assert!(
            graph
                .subgraph(&Id::new("b").unwrap(), &[Id::new("a").unwrap()])
                .is_err()
        );
    }
    #[test]
    fn cycles_missing_edges_and_duplicate_ids_rejected() {
        for tasks in [
            vec![
                task("a", Phase::Generate, &["b"]),
                task("b", Phase::Generate, &["a"]),
            ],
            vec![task("a", Phase::Generate, &["missing"])],
            vec![
                task("a", Phase::Metadata, &[]),
                task("a", Phase::Metadata, &[]),
            ],
        ] {
            assert!(TaskGraph::new(tasks).is_err());
        }
    }
    #[test]
    fn independent_provider_compilation_can_precede_metadata() {
        let graph = TaskGraph::new(vec![
            task("compile-provider", Phase::Compile, &[]),
            task("metadata", Phase::Metadata, &["compile-provider"]),
            task("generate", Phase::Generate, &["metadata"]),
        ])
        .unwrap();
        assert_eq!(
            graph
                .tasks()
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            ["compile-provider", "metadata", "generate"]
        );
        assert!(
            graph
                .reject_active(&[Id::new("metadata").unwrap()])
                .is_err()
        );
    }

    #[test]
    fn output_ownership_rejects_aliases_and_ancestor_paths() {
        for (left, right) in [
            ("out/File.h", "out/file.h"),
            ("Out/a", "out/b"),
            ("out/A/x", "out/a/y"),
            ("out/api/header.h", "out/api"),
            ("out/api", "out/api/header.h"),
            ("same", "same"),
        ] {
            let mut a = task("a", Phase::Generate, &[]);
            let mut b = task("b", Phase::Generate, &[]);
            a.outputs.push(LogicalPath::new(left).unwrap());
            b.outputs.push(LogicalPath::new(right).unwrap());
            assert!(TaskGraph::new(vec![a, b]).is_err());
        }
    }
}
