#[cfg(test)]
mod tests {
    use crate::model::{parse_range, parse_category, parse_item, suggest_next_code, Tree, Node, NodeType};

    #[test]
    fn parse_variants() {
        assert_eq!(parse_range("30-39_Research").unwrap().0, "30-39");
        assert_eq!(parse_category("30_Topic").unwrap().0, "30");
        assert_eq!(parse_item("30.01_Something.txt").unwrap().0, "30.01");
    }

    #[test]
    fn suggest_basic() {
        let t = Tree { roots: vec![Node { id: "r".into(), code: Some("30".into()), title: "30".into(), path: "".into(), node_type: NodeType::Category, children: vec![
            Node { id: "a".into(), code: Some("30.01".into()), title: "a".into(), path: "".into(), node_type: NodeType::ItemDir, children: vec![] },
        ] }] };
        let next = suggest_next_code(&t, "30").unwrap();
        assert_eq!(next, "30.02");
    }
}


