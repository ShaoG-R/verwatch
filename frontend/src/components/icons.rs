//! 图标组件模块
//!
//! 使用宏简化 SVG 图标组件的定义。
//! Silex 版本：利用组件的属性透传功能实现 SVG 图标。

use silex::prelude::*;

/// 定义 SVG 图标组件的宏
///
/// 自动生成带有标准 SVG 属性的 Silex 组件。
/// 返回 Element 类型以支持属性透传 (.style(), .class(), .on() 等)。
macro_rules! icon {
    ($name:ident, $($inner:tt)*) => {
        #[component]
        pub fn $name() -> Element {
            svg(
                ($($inner)*)
            )
            .attr("xmlns", "http://www.w3.org/2000/svg")
            .attr("width", "24")
            .attr("height", "24")
            .attr("viewBox", "0 0 24 24")
            .attr("fill", "none")
            .attr("stroke", "currentColor")
            .attr("stroke-width", "2")
            .attr("stroke-linecap", "round")
            .attr("stroke-linejoin", "round")
        }
    };
}

// --- 图标定义 ---

icon!(ShieldCheck,
    path()
        .attr("d", "M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"),
    path().attr("d", "m9 12 2 2 4-4")
);

icon!(
    Radio,
    path().attr("d", "M4.9 19.1C1 15.2 1 8.8 4.9 4.9"),
    path().attr("d", "M7.8 16.2c-2.3-2.3-2.3-6.1 0-8.5"),
    circle().attr("cx", "12").attr("cy", "12").attr("r", "2"),
    path().attr("d", "M16.2 7.8c2.3 2.3 2.3 6.1 0 8.5"),
    path().attr("d", "M19.1 4.9C23 8.8 23 15.1 19.1 19")
);

icon!(
    LogOut,
    path().attr("d", "M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"),
    polyline().attr("points", "16 17 21 12 16 7"),
    line()
        .attr("x1", "21")
        .attr("x2", "9")
        .attr("y1", "12")
        .attr("y2", "12")
);

icon!(
    Plus,
    path().attr("d", "M5 12h14"),
    path().attr("d", "M12 5v14")
);

icon!(
    MoreHorizontal,
    circle().attr("cx", "12").attr("cy", "12").attr("r", "1"),
    circle().attr("cx", "19").attr("cy", "12").attr("r", "1"),
    circle().attr("cx", "5").attr("cy", "12").attr("r", "1")
);

icon!(
    Trash2,
    path().attr("d", "M3 6h18"),
    path().attr("d", "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"),
    path().attr("d", "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"),
    line()
        .attr("x1", "10")
        .attr("x2", "10")
        .attr("y1", "11")
        .attr("y2", "17"),
    line()
        .attr("x1", "14")
        .attr("x2", "14")
        .attr("y1", "11")
        .attr("y2", "17")
);

icon!(Github,
    path().attr("d", "M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.403 5.403 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65-.17.6-.22 1.23-.15 1.85v4"),
    path().attr("d", "M9 18c-4.51 2-5-2-7-2")
);

icon!(
    GitFork,
    circle().attr("cx", "12").attr("cy", "18").attr("r", "3"),
    circle().attr("cx", "6").attr("cy", "6").attr("r", "3"),
    circle().attr("cx", "18").attr("cy", "6").attr("r", "3"),
    path().attr("d", "M18 9v1a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V9"),
    path().attr("d", "M12 12v3")
);

icon!(
    RefreshCw,
    path().attr("d", "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"),
    path().attr("d", "M21 3v5h-5"),
    path().attr("d", "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"),
    path().attr("d", "M8 16H3v5")
);

icon!(
    Pause,
    rect()
        .attr("x", "6")
        .attr("y", "4")
        .attr("width", "4")
        .attr("height", "16"),
    rect()
        .attr("x", "14")
        .attr("y", "4")
        .attr("width", "4")
        .attr("height", "16")
);

icon!(Play, polygon().attr("points", "5 3 19 12 5 21 5 3"));

icon!(
    Clock,
    circle().attr("cx", "12").attr("cy", "12").attr("r", "10"),
    polyline().attr("points", "12 6 12 12 16 14")
);
