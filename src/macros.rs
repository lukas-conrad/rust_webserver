#[macro_export]
macro_rules! cloned {
    ($($name:ident),*; $block:expr) => {{
        $(let $name = $name.clone();)*
        $block
    }};
}
#[macro_export]
macro_rules! spawn_cloned {
    ($($name:ident),*; async move $block:block) => {{
        $(let $name = $name.clone();)*
        tokio::spawn(async move $block)
    }};

    ($($name:ident),*; async $block:block) => {{
        $(let $name = $name.clone();)*
        tokio::spawn(async move $block)
    }};

    (async move $block:block) => {{
        tokio::spawn(async move $block)
    }};

    (move $block:block) => {{
        tokio::spawn(async move $block)
    }};
}
