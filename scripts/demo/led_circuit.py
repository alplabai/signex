"""The LED indicator the schematic story draws and the automation story
starts from: a two-pin connector, a series resistor and an LED, returned to
ground. Each entry is one step: a caption and the edits of one undo step."""

from demo_lib import STOCK_LIBRARY


def part(name, at, **extra):
    return ("place_component", {"library": STOCK_LIBRARY, "part": name, "at": at, **extra})


def wire(start, end, **extra):
    return ("place_wire", {"start": start, "end": end, **extra})


def ground(at):
    return ("place_power_symbol", {"at": at, "net_name": "GND", "lib_id": "power:GND"})


STEPS = [
    (
        "Place a two-pin connector from the stock library",
        [part("STOCK-Conn_1x02", [101.6, 101.6], mirror="y")],
    ),
    (
        "Add a 470 R resistor; parts are numbered as they are placed",
        [part("STOCK-R", [127.0, 100.33], rotation=90, value="470R")],
    ),
    ("Add a green LED", [part("STOCK-LED", [149.86, 100.33], rotation=90, value="Green")]),
    ("Wire the connector to the resistor, pin to pin", [wire("J1.1", "R1.1")]),
    ("Wire the resistor to the LED", [wire("R1.2", "D1.2")]),
    (
        "Bring the return path down to ground",
        [
            wire("D1.1", [162.56, 100.33], allow_open=True),
            wire([162.56, 100.33], [162.56, 107.95], allow_open=True),
            wire("J1.2", [111.76, 102.87], allow_open=True),
            wire([111.76, 102.87], [111.76, 107.95], allow_open=True),
        ],
    ),
    ("Add GND power ports", [ground([162.56, 107.95]), ground([111.76, 107.95])]),
    ("Name the supply net VIN", [("place_net_label", {"at": [116.84, 100.33], "text": "VIN"})]),
]

# A point on each net, for commands that pick a net by position.
NET_POINTS = {
    "VIN": [116.84, 100.33],
    "LED_A": [139.7, 100.33],
    "GND": [162.56, 104.14],
}
