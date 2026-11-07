//! Disjoint sets over `0..len`, for grouping connected copper.

/// Union-find with path halving and union by rank.
#[derive(Debug, Clone, Default)]
pub struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    /// `len` singleton sets.
    pub fn new(len: usize) -> Self {
        UnionFind {
            parent: (0..len).collect(),
            rank: vec![0; len],
        }
    }

    pub fn len(&self) -> usize {
        self.parent.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// The representative of `x`'s set.
    ///
    /// # Panics
    /// If `x >= len()`.
    pub fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Join the sets of `a` and `b`; `true` if they were apart.
    ///
    /// # Panics
    /// If either index is `>= len()`.
    pub fn union(&mut self, a: usize, b: usize) -> bool {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return false;
        }
        match self.rank[ra].cmp(&self.rank[rb]) {
            std::cmp::Ordering::Less => self.parent[ra] = rb,
            std::cmp::Ordering::Greater => self.parent[rb] = ra,
            std::cmp::Ordering::Equal => {
                self.parent[rb] = ra;
                self.rank[ra] = self.rank[ra].saturating_add(1);
            }
        }
        true
    }

    pub fn same(&mut self, a: usize, b: usize) -> bool {
        self.find(a) == self.find(b)
    }

    /// Every set as its sorted members, ordered by smallest member.
    pub fn groups(&mut self) -> Vec<Vec<usize>> {
        let mut by_root: Vec<Option<usize>> = vec![None; self.len()];
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for x in 0..self.len() {
            let root = self.find(x);
            let slot = *by_root[root].get_or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
            groups[slot].push(x);
        }
        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_as_singletons() {
        let mut uf = UnionFind::new(3);
        assert_eq!(uf.len(), 3);
        assert!(!uf.same(0, 1));
        assert_eq!(uf.groups(), vec![vec![0], vec![1], vec![2]]);
        assert!(UnionFind::new(0).is_empty());
    }

    #[test]
    fn union_is_transitive_and_reports_merges() {
        let mut uf = UnionFind::new(6);
        assert!(uf.union(0, 4));
        assert!(uf.union(4, 2));
        assert!(!uf.union(2, 0));
        assert!(uf.union(5, 3));
        assert!(uf.same(0, 2));
        assert!(!uf.same(0, 3));
        assert_eq!(uf.groups(), vec![vec![0, 2, 4], vec![1], vec![3, 5]]);
    }

    #[test]
    fn a_long_chain_flattens() {
        let mut uf = UnionFind::new(1000);
        for i in 1..1000 {
            uf.union(i - 1, i);
        }
        let root = uf.find(0);
        assert!((0..1000).all(|i| uf.find(i) == root));
        assert_eq!(uf.groups().len(), 1);
    }
}
