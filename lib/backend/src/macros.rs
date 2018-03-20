/// Helper macro to implement listeners opt loop.
#[macro_export]
macro_rules! code {
    ($codes:expr, $context:path) => {{
        let mut t = Tokens::new();

        for c in $codes {
            if let $context { .. } = c.context {
                t.append({
                    let mut t = Tokens::new();

                    for line in &c.lines {
                        t.push(line.as_str());
                    }

                    t
                });
            }
        }

        t
    }}
}
