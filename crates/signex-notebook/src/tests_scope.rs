//! Scope captures (mockup row 199): the CSV read, the SI numbers, and the
//! capture drawn as a 10 x 8 division vector plot with its legend,
//! cursors, readout and trigger.

use crate::attachment::attachments;
use crate::scope::{
    Coupling, Cursors, DIVISIONS_X, PlotSpec, ScopeError, Trigger, parse_csv, plot,
};
use crate::si;

const T: char = '\u{2009}';

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12_f64.max(b.abs() * 1e-9)
}

/// 10 µs of a 500 kHz ripple of ±9 mV on 3.3 V, 1001 samples.
fn ripple_csv() -> String {
    let mut csv = String::from("Model,BENCH-200\nTime (s),CH1 (V)\n");
    for i in 0..=1000 {
        let t = f64::from(i) * 1e-8;
        let v = 3.3 + 0.009 * (2.0 * std::f64::consts::PI * 5e5 * t).sin();
        csv.push_str(&format!("{t:e},{v}\n"));
    }
    csv
}

fn ripple_spec() -> PlotSpec {
    let text = "#attachment(\"scope/r.csv\", channels: ((name: \"CH1\", label: \"+3V3\", coupling: \"AC\", scale: \"5 mV\", bandwidth: \"20 MHz\"),), cursors: (y1: \"9 mV\", y2: \"-9 mV\", x1: \"1 µs\", x2: 3e-6))\n";
    attachments(text).0.remove(0).plot
}

#[test]
fn si_numbers_are_read_with_their_prefix() {
    let read = |text: &str| si::parse(text).expect(text);
    let (v, u) = read("5 mV");
    assert!(close(v, 0.005) && u == "V");
    let (v, u) = read("20MHz");
    assert!(close(v, 2e7) && u == "Hz");
    let (v, u) = read("\u{2212}9 mV");
    assert!(close(v, -0.009) && u == "V");
    let (v, u) = read("1e-6 s");
    assert!(close(v, 1e-6) && u == "s");
    let (v, u) = read("4.7uH");
    assert!(close(v, 4.7e-6) && u == "H");
    assert_eq!(read("3.3"), (3.3, String::new()));
    let lone = read("5 m");
    assert_eq!(lone, (5.0, "m".to_string()), "a lone prefix is the unit");
    assert_eq!(si::parse("about 5"), None);
}

#[test]
fn si_numbers_are_written_with_a_prefix_and_a_thin_space() {
    assert_eq!(si::format(0.005, "V", 3), format!("5{T}mV"));
    assert_eq!(si::format(5e5, "Hz", 3), format!("500{T}kHz"));
    assert_eq!(si::format(0.0, "V", 3), format!("0{T}V"));
    assert_eq!(si::format(-0.015, "V", 3), format!("\u{2212}15{T}mV"));
    assert_eq!(si::format(1234.0, "Ω", 3), format!("1.23{T}kΩ"));
    assert_eq!(si::number(-15.0, 3), "\u{2212}15");
    assert_eq!(si::number(0.5, 3), "0.5");
    assert_eq!(si::prefix_for(0.015), (1e-3, "m"));
    assert_eq!(si::prefix_for(0.0), (1.0, ""));
    assert_eq!(si::prefix_for(12.0), (1.0, ""));
}

#[test]
fn a_scale_is_the_next_1_2_5_step() {
    let steps = [
        (0.3, 0.5),
        (1.0, 1.0),
        (0.2, 0.2),
        (7.0, 10.0),
        (1.1e-6, 2e-6),
    ];
    for (raw, step) in steps {
        let got = si::step_125(raw);
        assert!(close(got, step), "{raw} -> {got}");
    }
    assert_eq!(si::step_125(0.0), 1.0);
    assert_eq!(si::step_125(f64::NAN), 1.0);
}

#[test]
fn a_csv_with_names_and_units_in_the_header_is_read() {
    let capture = parse_csv(&ripple_csv()).expect("a capture");
    assert_eq!(capture.time_unit, "s");
    assert_eq!(capture.times.len(), 1001);
    assert_eq!(capture.channels.len(), 1);
    assert_eq!(capture.channels[0].name, "CH1");
    assert_eq!(capture.channels[0].unit, "V");
}

#[test]
fn a_units_line_spelled_out_and_a_decimal_comma_are_read() {
    let capture = parse_csv("X,CH1,CH2\nSecond,Volt,Volt\n0,1,2\n1,3,4\n").expect("read");
    assert_eq!(capture.time_unit, "s");
    assert_eq!(capture.channels[1].name, "CH2");
    assert_eq!(capture.channels[1].unit, "V");
    let capture = parse_csv("# exported\nt;v\ns;mV\n0,5;1,25\n1;2\n").expect("read");
    assert_eq!(capture.channels[0].unit, "mV");
    assert_eq!(capture.times, [0.5, 1.0]);
    assert_eq!(capture.channels[0].samples, [1.25, 2.0]);
    let bare = parse_csv("0\t1\n1\t2\n").expect("read");
    let channel = &bare.channels[0];
    assert_eq!((channel.name.as_str(), channel.unit.as_str()), ("CH1", "V"));
}

#[test]
fn a_csv_that_is_not_a_capture_says_where() {
    assert_eq!(parse_csv("only,names\n"), Err(ScopeError::Empty));
    assert_eq!(
        parse_csv("t,v\n0,1\n1,2,3\n"),
        Err(ScopeError::Ragged {
            line: 3,
            found: 3,
            expected: 2
        })
    );
    assert_eq!(
        parse_csv("t,v\n0,1\n1,x\n"),
        Err(ScopeError::NotANumber { line: 3, column: 2 })
    );
}

#[test]
fn the_attachment_says_how_its_capture_is_drawn() {
    let spec = ripple_spec();
    let channel = &spec.channels[0];
    assert_eq!(channel.name, "CH1");
    assert_eq!(channel.coupling, Coupling::Ac);
    assert!(close(channel.scale.unwrap(), 0.005));
    assert_eq!(channel.bandwidth, "20 MHz");
    let c = spec.cursors;
    assert!(close(c.y1.unwrap(), 0.009) && close(c.y2.unwrap(), -0.009));
    assert!(close(c.x1.unwrap(), 1e-6) && close(c.x2.unwrap(), 3e-6));
    assert_eq!(spec.trigger, None);
}

#[test]
fn the_ripple_is_drawn_as_on_the_scope() {
    let capture = parse_csv(&ripple_csv()).expect("a capture");
    let drawn = plot(&capture, &ripple_spec());
    assert_eq!(drawn.time_per_div, format!("1{T}µs/div"));
    let channel = &drawn.channels[0];
    let legend = format!("CH1 +3V3 AC · 5{T}mV/div · 20 MHz BW");
    assert_eq!(channel.legend, legend);
    let centre = channel.centre;
    assert!((centre - 3.3).abs() < 1e-4, "AC: around its mean: {centre}");
    let points = channel.points.len();
    assert!(points <= 800, "decimated: {points}");
    let highest = channel.points.iter().map(|p| p.1).fold(f64::MIN, f64::max);
    assert!((highest - 1.8).abs() < 0.05, "9 mV at 5 mV/div: {highest}");
    let readout = [
        format!("ΔY = 18{T}mV p-p"),
        format!("ΔX = 2{T}µs"),
        format!("f = 500{T}kHz"),
    ];
    assert_eq!(drawn.readout, readout);
    let x: Vec<&str> = drawn.x_labels.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(x, ["0", "2", "4", "6", "8", &format!("10{T}µs")]);
    let y: Vec<&str> = drawn.y_labels.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(y, ["15", "0", &format!("\u{2212}15{T}mV")]);
    assert_eq!(drawn.trigger, Some(0.0), "it starts at the trigger");
    let labels: Vec<&str> = drawn.cursors.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["Y1", "Y2", "X1", "X2"]);
    assert!(close(drawn.cursors[0].at, 1.8));
    assert!(close(drawn.cursors[2].at, 1.0));
}

#[test]
fn a_dc_channel_with_no_scale_fits_the_screen() {
    let capture = parse_csv("t,v\n-1,0\n0,1.65\n1,3.3\n").expect("read");
    let spec = PlotSpec {
        trigger: Some(Trigger { time: 0.5 }),
        cursors: Cursors::default(),
        ..PlotSpec::default()
    };
    let drawn = plot(&capture, &spec);
    let per_div = drawn.channels[0].per_div;
    assert!(close(per_div, 2.0), "3.3 V in three divisions: {per_div}");
    assert_eq!(drawn.channels[0].coupling, Coupling::Dc);
    let trigger = drawn.trigger.unwrap();
    assert!(close(trigger, 1.5 / 0.2), "0.5 s at 0.2 s/div: {trigger}");
    assert!(drawn.readout.is_empty());
    let spec = PlotSpec {
        trigger: Some(Trigger { time: 99.0 }),
        ..PlotSpec::default()
    };
    assert_eq!(plot(&capture, &spec).trigger, None, "off screen");
    assert!(f64::from(DIVISIONS_X) > 0.0);
}

#[test]
fn the_plot_is_an_svg_of_lines_with_the_same_bytes_every_time() {
    let capture = parse_csv(&ripple_csv()).expect("a capture");
    let svg = plot(&capture, &ripple_spec()).to_svg();
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 400 320\""));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("<polyline"), "the trace");
    assert_eq!(svg.matches("stroke-dasharray").count(), 4, "four cursors");
    assert!(!svg.contains("<text"), "the package writes the labels");
    assert_eq!(svg, plot(&capture, &ripple_spec()).to_svg());
}
