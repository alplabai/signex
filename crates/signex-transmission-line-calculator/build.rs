use std::env;
use std::fs;
use std::path::PathBuf;

use mathjax_svg_rs::{Options, render_tex};

const EQUATION_FONT_SIZE_PX: f64 = 16.0;

const EQUATIONS: [&str; 7] = [
    r"L_m = k\sqrt{L_1L_2}",
    r"Z_{in}=Z_0\frac{Z_L+jZ_0\tan(\beta l)}{Z_0+jZ_L\tan(\beta l)},\quad \beta=\frac{2\pi}{\lambda}",
    r"Z=-j\frac{Z_0}{\tan(\beta l)}",
    r"Z=-j\frac{Z_0}{\tan(\beta l+\pi/2)},\quad \cot(x+\pi/2)=-\tan(x)",
    r"G_{TU}=G_S\left|S_{21}\right|^2G_L,\quad Z=Z_0\frac{1+\Gamma}{1-\Gamma}",
    r"N=\frac{F-F_{min}}{4R_n/Z_0}\left|1+\Gamma_{opt}\right|^2,\quad C_F=\frac{\Gamma_{opt}}{N+1}",
    r"\Delta=S_{11}S_{22}-S_{12}S_{21}",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let output_directory = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set"));
    let options = Options {
        font_size: EQUATION_FONT_SIZE_PX,
        ..Options::default()
    };
    for (index, equation) in EQUATIONS.iter().enumerate() {
        let svg = render_tex(equation, &options)
            .unwrap_or_else(|error| panic!("failed to render equation {index}: {error}"));
        fs::write(
            output_directory.join(format!("smith_chart_equation_{index}.svg")),
            svg,
        )
        .unwrap_or_else(|error| panic!("failed to write equation {index}: {error}"));
    }
}
