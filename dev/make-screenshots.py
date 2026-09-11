"""
Draws the SVG pictures of the interface for the README.

Hand drawn, not photographed: they stay sharp at any size, weigh a few
kilobytes, and a change shows up in a diff. The sources under
assets/screenshots/src are drawn in the dark theme; this script recolours each
one into the light theme and puts both halves into one picture, torn apart
along a ragged diagonal, so a single image shows both looks.

    python3 dev/make-screenshots.py

Until the panes actually list files — milestone M1 — the layout picture shows
what is planned, and the README says so.
"""

from pathlib import Path

W, H = 1200, 720

BG = "#0d0f14"
PANEL = "#151821"
PANEL_2 = "#1b1f2a"
PANEL_3 = "#232837"
BORDER = "#262c3a"
BORDER_STRONG = "#343b4d"
TEXT = "#e8eaf0"
MUTED = "#9aa2b5"
FAINT = "#6b7386"
ACCENT = "#e08b12"
ACCENT_SOFT = "#33270e"
OK = "#35c489"
FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, sans-serif"
MONO = "ui-monospace, SFMono-Regular, Menlo, monospace"


def text(x, y, value, fill=TEXT, size=12.5, weight=None, anchor=None, family=None, opacity=None):
    parts = [f'<text x="{x}" y="{y}" fill="{fill}" font-size="{size}"']
    if weight:
        parts.append(f' font-weight="{weight}"')
    if anchor:
        parts.append(f' text-anchor="{anchor}"')
    if family:
        parts.append(f' font-family="{family}"')
    if opacity:
        parts.append(f' opacity="{opacity}"')
    parts.append(f">{value}</text>")
    return "".join(parts)


def rect(x, y, w, h, fill, r=0, stroke=None):
    extra = f' stroke="{stroke}" stroke-width="1"' if stroke else ""
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}"{extra}/>'


def line(x1, y1, x2, y2, stroke=BORDER):
    return f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="1"/>'


def head(label, title, desc, height=H):
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {height}" width="{W}" height="{height}" role="img" aria-label="{label}">
  <title>{title}</title>
  <desc>{desc}</desc>

  <defs>
    <linearGradient id="accent" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#f7a824"/>
      <stop offset="1" stop-color="#c4720a"/>
    </linearGradient>
    <clipPath id="round">
      <rect x="0" y="0" width="{W}" height="{height}" rx="14"/>
    </clipPath>
  </defs>

  <g clip-path="url(#round)" font-family="{FONT}">
    {rect(0, 0, W, height, BG)}
"""


def tail():
    return "  </g>\n</svg>\n"


def titlebar(subtitle):
    return f"""
    <!-- title bar -->
    {rect(0, 0, W, 44, PANEL)}
    {line(0, 44, W, 44)}
    <circle cx="24" cy="22" r="6" fill="#ff5f57"/>
    <circle cx="44" cy="22" r="6" fill="#febc2e"/>
    <circle cx="64" cy="22" r="6" fill="#28c840"/>
    {text(94, 26, "AmberBeam", MUTED, 12.5, "500")}
    {text(172, 26, subtitle, FAINT, 12.5)}
"""


def section_bar(x, y, w, label, hint):
    """The strip above each of the three regions."""
    return f"""
    {rect(x, y, w, 26, PANEL_2)}
    {line(x, y + 26, x + w, y + 26)}
    {text(x + 12, y + 17, label, MUTED, 11, "600")}
    {text(x + w - 12, y + 17, hint, FAINT, 10.5, anchor="end", family=MONO)}
"""


def folder_icon(x, y, colour):
    return (
        f'<path d="M{x} {y + 9}v-7a1.6 1.6 0 0 1 1.6-1.6h3.4l1.6 2h5.8a1.6 1.6 0 0 1 1.6 1.6v5'
        f'a1.6 1.6 0 0 1-1.6 1.6H{x + 1.6}A1.6 1.6 0 0 1 {x} {y + 9}z" fill="none" '
        f'stroke="{colour}" stroke-width="1.4" stroke-linejoin="round"/>'
    )


def file_icon(x, y, colour):
    return (
        f'<path d="M{x + 1} {y}h6l4 4v7a1.4 1.4 0 0 1-1.4 1.4H{x + 1}A1.4 1.4 0 0 1 {x - 0.4} '
        f'{y + 11}V{y + 1.4}A1.4 1.4 0 0 1 {x + 1} {y}z" fill="none" stroke="{colour}" '
        f'stroke-width="1.4" stroke-linejoin="round"/>'
    )


def pane(x, y, w, h, title, place, tree, rows, active_row=None, accent_title=False):
    """One file region: path bar on top, tree on the left, list on the right."""
    tree_w = 168
    out = [rect(x, y, w, h, PANEL)]
    out.append(section_bar(x, y, w, title, place))
    head_y = y + 26

    # path bar
    out.append(rect(x, head_y, w, 30, PANEL))
    out.append(line(x, head_y + 30, x + w, head_y + 30))
    out.append(rect(x + 12, head_y + 6, w - 24, 19, PANEL_3, 5))
    out.append(text(x + 22, head_y + 19, place, MUTED, 11, family=MONO))

    body_y = head_y + 30

    # tree
    out.append(rect(x, body_y, tree_w, y + h - body_y, PANEL_2))
    out.append(line(x + tree_w, body_y, x + tree_w, y + h))
    ty = body_y + 20
    for depth, name, open_ in tree:
        colour = ACCENT if open_ else MUTED
        arrow = "▾" if open_ else "▸"
        out.append(text(x + 12 + depth * 14, ty, arrow, FAINT, 10))
        out.append(folder_icon(x + 24 + depth * 14, ty - 9, colour))
        out.append(text(x + 42 + depth * 14, ty, name, colour if open_ else MUTED, 11.5))
        ty += 22

    # list
    list_x = x + tree_w
    list_w = w - tree_w
    out.append(rect(list_x, body_y, list_w, 24, PANEL_2))
    out.append(line(list_x, body_y + 24, x + w, body_y + 24))
    for label, offset, anchor in (("Name", 14, None), ("Größe", list_w - 118, "end"),
                                  ("Geändert", list_w - 14, "end")):
        out.append(text(list_x + offset, body_y + 16, label, FAINT, 9.5, "600", anchor))

    ry = body_y + 24
    for index, (name, size, when, is_dir) in enumerate(rows):
        if index == active_row:
            out.append(rect(list_x, ry, list_w, 26, ACCENT_SOFT))
            out.append(rect(list_x, ry, 2, 26, ACCENT))
        colour = TEXT if index == active_row else MUTED
        icon = folder_icon if is_dir else file_icon
        out.append(icon(list_x + 14, ry + 7, ACCENT if is_dir else FAINT))
        out.append(text(list_x + 36, ry + 17, name, colour, 11.5))
        out.append(text(list_x + list_w - 118, ry + 17, size, FAINT, 11, family=MONO, anchor="end"))
        out.append(text(list_x + list_w - 14, ry + 17, when, FAINT, 11, family=MONO, anchor="end"))
        out.append(line(list_x, ry + 26, x + w, ry + 26))
        ry += 26

    if accent_title:
        out.append(rect(x, y, w, 2, ACCENT))
    return "".join(out)


def layout():
    """The whole window: log on top, two panes, queue at the bottom."""
    out = [head(
        "AmberBeam window with the server log on top, two file panes in the middle and the "
        "transfer queue at the bottom, dark and light side by side",
        "AmberBeam — Fensterlayout",
        "Server log on top, local files on the left, the server on the right, the transfer "
        "queue at the bottom.",
    )]
    out.append(titlebar("beispiel.de · SFTP"))

    # server log
    log_y = 44
    log_h = 116
    out.append(rect(0, log_y, W, log_h, PANEL))
    out.append(section_bar(0, log_y, W, "SERVER-LOG", "F4  Raw-Befehle"))
    log_lines = [
        ("17:02:11", "&#8594;", "MLSD /var/www/html", MUTED),
        ("17:02:11", "&#8592;", "150 Opening BINARY mode data connection", FAINT),
        ("17:02:12", "&#8592;", "226 Directory send OK", OK),
        ("17:02:19", "&#8594;", "STOR /var/www/html/stil.css", MUTED),
    ]
    ly = log_y + 44
    for stamp, arrow, message, colour in log_lines:
        out.append(text(14, ly, stamp, FAINT, 11, family=MONO))
        out.append(text(74, ly, arrow, ACCENT, 11, family=MONO))
        out.append(text(96, ly, message, colour, 11, family=MONO))
        ly += 20
    out.append(line(0, log_y + log_h, W, log_y + log_h))

    # two panes
    pane_y = log_y + log_h
    pane_h = 384
    out.append(pane(
        0, pane_y, 600, pane_h, "LOKAL", "~/Projekte/website",
        [(0, "Benutzer", True), (1, "dennis", True), (2, "Projekte", True), (3, "website", True),
         (3, "archiv", False), (2, "Bilder", False), (2, "Musik", False)],
        [("index.html", "4,2 KB", "11.09. 16:41", False),
         ("stil.css", "18 KB", "11.09. 16:38", False),
         ("skript.js", "7,1 KB", "10.09. 22:04", False),
         ("impressum.html", "3,4 KB", "10.09. 21:50", False),
         ("favicon.ico", "15 KB", "02.08. 11:07", False),
         ("liesmich.md", "1,9 KB", "28.07. 14:22", False),
         ("bilder", "—", "09.09. 19:12", True),
         ("schriften", "—", "09.09. 19:12", True),
         ("entwuerfe", "—", "14.07. 08:55", True)],
        active_row=1,
    ))
    out.append(line(600, pane_y, 600, pane_y + pane_h, BORDER_STRONG))
    out.append(pane(
        600, pane_y, 600, pane_h, "SERVER", "/var/www/html",
        [(0, "/", True), (1, "etc", False), (1, "srv", False), (1, "var", True),
         (2, "log", False), (2, "www", True), (3, "html", True)],
        [("index.html", "3,9 KB", "11.09. 09:20", False),
         ("stil.css", "17 KB", "09.09. 22:14", False),
         ("skript.js", "7,1 KB", "10.09. 22:04", False),
         (".htaccess", "612 B", "02.08. 11:07", False),
         ("robots.txt", "104 B", "02.08. 11:07", False),
         ("bilder", "—", "09.09. 19:12", True),
         ("alt", "—", "14.07. 08:55", True),
         ("logs", "—", "11.09. 17:00", True),
         ("tmp", "—", "11.09. 16:59", True)],
        accent_title=True,
    ))
    out.append(line(0, pane_y + pane_h, W, pane_y + pane_h))

    # queue
    q_y = pane_y + pane_h
    q_h = H - q_y - 26
    out.append(rect(0, q_y, W, q_h, PANEL))
    out.append(section_bar(0, q_y, W, "WARTESCHLANGE", "F8  ein/aus     F9  starten"))
    jobs = [
        ("&#8593;", "stil.css", "/var/www/html/", 1.0, "fertig", OK),
        ("&#8593;", "bilder/logo.svg", "/var/www/html/bilder/", 0.62, "62 %  ·  1,8 MB/s", ACCENT),
        ("&#8595;", "logs/error.log", "~/Projekte/website/", 0.0, "wartet", FAINT),
    ]
    jy = q_y + 38
    for arrow, name, target, done, note, colour in jobs:
        out.append(text(16, jy + 13, arrow, colour, 12, "600", family=MONO))
        out.append(text(38, jy + 13, name, TEXT if done else MUTED, 11.5))
        out.append(text(200, jy + 13, target, FAINT, 11, family=MONO))
        out.append(rect(470, jy + 5, 560, 8, PANEL_3, 4))
        if done > 0:
            out.append(rect(470, jy + 5, int(560 * done), 8, colour, 4))
        out.append(text(W - 16, jy + 13, note, colour if done else FAINT, 10.5,
                        family=MONO, anchor="end"))
        jy += 30

    # status bar
    out.append(rect(0, H - 26, W, 26, PANEL_2))
    out.append(line(0, H - 26, W, H - 26))
    keys = "F5 Aktualisieren   ·   F6 Fokus wechseln   ·   F7 Befehle   ·   F12 Verbinden"
    out.append(text(14, H - 9, keys, FAINT, 10.5, family=MONO))
    out.append(text(W - 14, H - 9, "SFTP  ·  8 gleichzeitig", OK, 10.5, family=MONO, anchor="end"))

    out.append(tail())
    return "".join(out)


LIGHT = {
    "#0d0f14": "#f3f5fa",  # window background
    "#151821": "#ffffff",  # panel
    "#1b1f2a": "#f6f8fc",  # panel, second level
    "#232837": "#e8ecf5",  # inner block
    "#262c3a": "#dde2ee",  # border
    "#343b4d": "#c8cfdd",  # border, strong
    "#e8eaf0": "#131722",  # text
    "#9aa2b5": "#4b5568",  # muted text
    "#6b7386": "#7a8397",  # faint text
    "#33270e": "#fdf0da",  # accent background
    "#35c489": "#15925f",  # ok
}

# The tear, as horizontal offsets every 18 pixels down the picture. Fixed on
# purpose: a random edge would make every run a new diff.
TEAR = [0, 4, -3, 6, -5, 2, -2, 7, -4, 1, 5, -6, 3, -2, 5, -3, 2, 4, -3, 3, -5, 2, 6, -4, 1, -6, 3, 5, -2]


def tear_points(height):
    """The ragged line, from the top edge down to the bottom edge."""
    top, bottom = int(W * 0.60), int(W * 0.40)
    points = []
    steps = max(height // 18, 2)
    for index in range(steps + 1):
        y = round(index * height / steps)
        x = round(top + (bottom - top) * index / steps) + TEAR[index % len(TEAR)]
        points.append((x, y))
    return points


def split(source):
    """Puts the dark and the light version of one picture into one file."""
    start = source.index(">", source.index('<g clip-path="url(#round')) + 1
    end = source.rindex("</g>")
    body_dark = source[start:end]

    body_light = body_dark
    for dark, light in LIGHT.items():
        body_light = body_light.replace(dark, light).replace(dark.upper(), light)

    height = int(source.split('height="', 2)[1].split('"')[0])
    points = tear_points(height)
    path = " ".join(f"{x} {y}" for x, y in points)
    left = f"M0 0 L{points[0][0]} 0 L{path} L0 {height} Z"
    right = f"M{W} 0 L{points[0][0]} 0 L{path} L{W} {height} Z"

    defs_end = source.index("</defs>")
    extra = (f'    <clipPath id="tear-dark"><path d="{left}"/></clipPath>\n'
             f'    <clipPath id="tear-light"><path d="{right}"/></clipPath>\n')
    return (
        source[:defs_end] + extra + source[defs_end:start]
        + f'\n    <g clip-path="url(#tear-dark)">{body_dark}</g>\n'
        + f'    <g clip-path="url(#tear-light)">{body_light}</g>\n'
        + f'    <path d="M{points[0][0]} 0 L{path}" fill="none" stroke="{ACCENT}" '
          'stroke-width="2" stroke-opacity="0.75" stroke-linejoin="round"/>\n'
        + source[end:]
    )


SRC = Path("assets/screenshots/src")
OUT = Path("assets/screenshots")


def main():
    SRC.mkdir(parents=True, exist_ok=True)
    (SRC / "layout.svg").write_text(layout())
    for source in sorted(SRC.glob("*.svg")):
        (OUT / source.name).write_text(split(source.read_text()))
    Path("assets/social-preview.svg").write_text(social())
    print("wrote", ", ".join(sorted(p.name for p in OUT.glob("*.svg"))), "and social-preview.svg")


# ---------------------------------------------------------------- social card

SW, SH = 1280, 640


def tick(y, label):
    return (
        f'<circle cx="82" cy="{y}" r="9" fill="none" stroke="{OK}" stroke-width="2.2"/>'
        f'<path d="M78 {y}l3 3 5.5-6" stroke="{OK}" stroke-width="2.4" fill="none" '
        'stroke-linecap="round" stroke-linejoin="round"/>'
        + text(104, y + 6, label, "#c8cdda", 17.5)
    )


def pill(x, y, w, label, accented=False, size=16):
    fill = ACCENT_SOFT if accented else PANEL_2
    stroke = "#6b4a12" if accented else "#2b3040"
    colour = "#f0b429" if accented else "#c8cdda"
    return (
        f'<rect x="{x}" y="{y}" width="{w}" height="34" rx="17" fill="{fill}" stroke="{stroke}"/>'
        + text(x + w / 2, y + 22, label, colour, size, "500", "middle")
    )


def mini_row(x, w, y, name, size, is_dir, selected=False):
    out = []
    if selected:
        out.append(rect(x, y, w, 26, ACCENT_SOFT))
        out.append(rect(x, y, 2, 26, ACCENT))
    icon = folder_icon if is_dir else file_icon
    out.append(icon(x + 12, y + 7, ACCENT if is_dir else FAINT))
    out.append(text(x + 32, y + 17, name, TEXT if selected else MUTED, 11))
    out.append(text(x + w - 12, y + 17, size, FAINT, 10, family=MONO, anchor="end"))
    return "".join(out)


def social():
    """The 1280x640 card GitHub shows wherever the repository is shared.

    The window is drawn large and flush with the right edge, and the text
    column's soft ground runs in under its left border — enough for the two to
    overlap, never enough to put a headline across a folder tree. A picture of
    a file transfer client has to show both panes with their trees, or it
    advertises a different program.
    """
    left_tree = [(0, "Benutzer", True), (1, "dennis", True), (2, "Projekte", True),
                 (3, "website", True), (3, "archiv", False), (2, "Bilder", False)]
    right_tree = [(0, "/", True), (1, "etc", False), (1, "var", True),
                  (2, "log", False), (2, "www", True), (3, "html", True)]
    left_rows = [
        ("index.html", "4,2 KB", False, False),
        ("stil.css", "18 KB", False, True),
        ("skript.js", "7,1 KB", False, False),
        ("impressum.html", "3,4 KB", False, False),
        ("favicon.ico", "15 KB", False, False),
        ("liesmich.md", "1,9 KB", False, False),
        ("bilder", "—", True, False),
        ("schriften", "—", True, False),
    ]
    right_rows = [
        ("index.html", "3,9 KB", False, False),
        ("stil.css", "17 KB", False, False),
        ("skript.js", "7,1 KB", False, False),
        (".htaccess", "612 B", False, False),
        ("robots.txt", "104 B", False, False),
        ("bilder", "—", True, False),
        ("alt", "—", True, False),
        ("logs", "—", True, False),
    ]

    out = []
    out.append(
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SW} {SH}" width="{SW}" '
        f'height="{SH}" role="img" aria-label="AmberBeam — dual-pane FTP and SFTP client for '
        'macOS, Windows, Linux and Docker">\n'
        "  <title>AmberBeam — dual-pane FTP and SFTP client</title>\n\n"
        "  <defs>\n"
        '    <linearGradient id="page" x1="0" y1="0" x2="1" y2="1">\n'
        '      <stop offset="0" stop-color="#12141c"/>\n'
        '      <stop offset="1" stop-color="#0a0b10"/>\n'
        "    </linearGradient>\n"
        '    <linearGradient id="brand" x1="0" y1="0" x2="0" y2="1">\n'
        '      <stop offset="0" stop-color="#f7a824"/>\n'
        '      <stop offset="1" stop-color="#c4720a"/>\n'
        "    </linearGradient>\n"
        '    <radialGradient id="glow" cx="0.5" cy="0.5" r="0.5">\n'
        f'      <stop offset="0" stop-color="{ACCENT}" stop-opacity="0.26"/>\n'
        f'      <stop offset="1" stop-color="{ACCENT}" stop-opacity="0"/>\n'
        "    </radialGradient>\n"
        # The window slides in behind the text column and fades out under it.
        # Full cover where the words sit, then a long ramp, so the overlap
        # reads as depth rather than as two pictures on top of each other.
        '    <linearGradient id="scrim" x1="0" y1="0" x2="1" y2="0">\n'
        '      <stop offset="0" stop-color="#0f1118" stop-opacity="1"/>\n'
        '      <stop offset="0.44" stop-color="#0f1118" stop-opacity="1"/>\n'
        '      <stop offset="0.56" stop-color="#0f1118" stop-opacity="0.55"/>\n'
        '      <stop offset="0.65" stop-color="#0f1118" stop-opacity="0"/>\n'
        "    </linearGradient>\n"
        '    <filter id="cardshadow" x="-30%" y="-30%" width="160%" height="160%">\n'
        '      <feDropShadow dx="0" dy="22" stdDeviation="34" flood-color="#000" '
        'flood-opacity="0.62"/>\n'
        "    </filter>\n"
        "  </defs>\n\n"
        f'  <g font-family="{FONT}">\n'
        + rect(0, 0, SW, SH, "url(#page)")
        + '<ellipse cx="1010" cy="150" rx="520" ry="380" fill="url(#glow)"/>\n'
    )

    # ---- the window, drawn first so the text column can lie over its edge
    wx, wy, ww, wh = 430, 48, 850, 552
    half = ww // 2
    tree_w = 155
    out.append(
        f'<g filter="url(#cardshadow)"><rect x="{wx}" y="{wy}" width="{ww}" height="{wh}" '
        f'rx="18" fill="{PANEL}" stroke="{BORDER}"/></g>'
    )
    out.append(
        f'<path d="M{wx} {wy + 18}a18 18 0 0 1 18-18h{ww - 36}a18 18 0 0 1 18 18v26H{wx}z" '
        f'fill="{PANEL_2}"/>'
        f'<circle cx="{wx + 26}" cy="{wy + 22}" r="6" fill="#ff5f57"/>'
        f'<circle cx="{wx + 46}" cy="{wy + 22}" r="6" fill="#febc2e"/>'
        f'<circle cx="{wx + 66}" cy="{wy + 22}" r="6" fill="#28c840"/>'
    )
    out.append(text(wx + 94, wy + 27, "AmberBeam", MUTED, 13, "500"))
    out.append(text(wx + 174, wy + 27, "beispiel.de · SFTP", FAINT, 13))
    out.append(line(wx, wy + 44, wx + ww, wy + 44))

    # server log
    log_y = wy + 44
    out.append(rect(wx, log_y, ww, 84, "#12151d"))
    out.append(text(wx + 16, log_y + 17, "SERVER-LOG", FAINT, 9.5, "600"))
    out.append(text(wx + ww - 16, log_y + 17, "F4  Raw-Befehle", FAINT, 9.5,
                    family=MONO, anchor="end"))
    for index, (stamp, arrow, message, colour) in enumerate((
        ("17:02:11", "&#8594;", "MLSD /var/www/html", MUTED),
        ("17:02:12", "&#8592;", "226 Directory send OK", OK),
        ("17:02:19", "&#8594;", "STOR /var/www/html/stil.css", MUTED),
    )):
        ly = log_y + 38 + index * 18
        out.append(text(wx + 16, ly, stamp, FAINT, 10.5, family=MONO))
        out.append(text(wx + 76, ly, arrow, ACCENT, 10.5, family=MONO))
        out.append(text(wx + 96, ly, message, colour, 10.5, family=MONO))
    pane_y = log_y + 84
    out.append(line(wx, pane_y, wx + ww, pane_y))

    # two panes, each with its own folder tree
    pane_h = 250
    for index, (label, place, tree, rows) in enumerate(
        (("LOKAL", "~/Projekte/website", left_tree, left_rows),
         ("SERVER", "/var/www/html", right_tree, right_rows))
    ):
        px = wx + index * half
        out.append(rect(px, pane_y, half, 22, PANEL_2))
        out.append(text(px + 14, pane_y + 15, label, FAINT, 9.5, "600"))
        out.append(text(px + half - 12, pane_y + 15, place, FAINT, 9.5,
                        family=MONO, anchor="end"))
        body_y = pane_y + 22
        out.append(line(px, body_y, px + half, body_y))

        out.append(rect(px, body_y, tree_w, pane_h - 22, PANEL_2))
        out.append(line(px + tree_w, body_y, px + tree_w, pane_y + pane_h))
        for depth, (level, name, open_) in enumerate(tree):
            ty = body_y + 20 + depth * 22
            out.append(text(px + 9 + level * 10, ty, "▾" if open_ else "▸", FAINT, 8))
            out.append(text(px + 20 + level * 10, ty, name, ACCENT if open_ else FAINT, 10))

        list_x = px + tree_w
        list_w = half - tree_w
        for row, (name, size, is_dir, selected) in enumerate(rows):
            ry = body_y + row * 26
            out.append(mini_row(list_x, list_w, ry, name, size, is_dir, selected))
            out.append(line(list_x, ry + 26, list_x + list_w, ry + 26))
    out.append(line(wx + half, pane_y, wx + half, pane_y + pane_h, BORDER_STRONG))
    queue_y = pane_y + pane_h
    out.append(line(wx, queue_y, wx + ww, queue_y))

    # queue
    out.append(rect(wx, queue_y, ww, 22, PANEL_2))
    out.append(text(wx + 14, queue_y + 15, "WARTESCHLANGE", FAINT, 9.5, "600"))
    out.append(text(wx + ww - 12, queue_y + 15, "F8  ein/aus     F9  starten", FAINT, 9.5,
                    family=MONO, anchor="end"))
    out.append(line(wx, queue_y + 22, wx + ww, queue_y + 22))
    for index, (arrow, name, target, done, note, colour) in enumerate((
        ("&#8593;", "stil.css", "/var/www/html/", 1.0, "fertig", OK),
        ("&#8593;", "bilder/logo.svg", "/var/www/html/bilder/", 0.62, "62 %", ACCENT),
        ("&#8595;", "logs/error.log", "~/Projekte/website/", 0.0, "wartet", FAINT),
    )):
        jy = queue_y + 34 + index * 30
        out.append(text(wx + 14, jy + 12, arrow, colour, 12, "600", family=MONO))
        out.append(text(wx + 34, jy + 12, name, MUTED, 11))
        out.append(text(wx + 190, jy + 12, target, FAINT, 10.5, family=MONO))
        out.append(rect(wx + 420, jy + 5, 330, 8, PANEL_3, 4))
        if done > 0:
            out.append(rect(wx + 420, jy + 5, int(330 * done), 8, colour, 4))
        out.append(text(wx + ww - 12, jy + 12, note, colour, 10, family=MONO, anchor="end"))

    # status strip
    status_y = wy + wh - 30
    out.append(rect(wx, status_y, ww, 30, PANEL_2))
    out.append(line(wx, status_y, wx + ww, status_y))
    out.append(text(wx + 14, status_y + 19, "F5 Aktualisieren  ·  F6 Fokus  ·  F12 Verbinden",
                    FAINT, 10, family=MONO))
    out.append(text(wx + ww - 12, status_y + 19, "SFTP  ·  8 gleichzeitig", OK, 10,
                    family=MONO, anchor="end"))

    # ---- the scrim, and the text column on top of it
    out.append(rect(0, 0, 900, SH, "url(#scrim)"))

    out.append('<rect x="72" y="72" width="76" height="76" rx="24" fill="url(#brand)"/>')
    out.append(
        '<g fill="none" stroke="#fff" stroke-width="4.2" stroke-linecap="round" '
        'stroke-linejoin="round">'
        '<path d="M92 110h32"/><path d="M117 100l10 10-10 10"/>'
        '<path d="M97 92h11" opacity="0.42"/><path d="M91 128h17" opacity="0.42"/></g>'
    )
    out.append(text(168, 106, "AmberBeam", TEXT, 30, "700"))
    out.append(text(168, 136, "Dual-pane FTP and SFTP", "#8a91a3", 18))
    out.append(text(72, 228, "Two panes.", "#ffffff", 44, "700"))
    out.append(text(72, 278, "The keyboard in charge.", "#ffffff", 44, "700"))
    out.append(text(72, 326, "Local and remote side by side,", MUTED, 19))
    out.append(text(72, 352, "with the folder tree where it belongs.", MUTED, 19))

    out.append(tick(404, "SFTP, FTP and FTPS"))
    out.append(tick(442, "Several files at once, set per server"))
    out.append(tick(480, "Resume a broken transfer mid-file"))
    out.append(tick(518, "Import sites from FileZilla and WinSCP"))

    out.append(pill(72, 562, 80, "macOS", size=15))
    out.append(pill(160, 562, 92, "Windows", size=15))
    out.append(pill(260, 562, 70, "Linux", size=15))
    out.append(pill(338, 562, 80, "Docker", accented=True, size=15))
    out.append(pill(426, 562, 96, "AGPL-3.0", size=15))

    out.append("\n  </g>\n</svg>\n")
    return "".join(out)


if __name__ == "__main__":
    main()
