from dataclasses import dataclass
from .core import shorten, cell_width


@dataclass
class Placement:
    node: object
    x: int
    y: int
    label: str


class Layout:
    """Shared horizontal/vertical tree in character-cell coordinates."""
    def __init__(self, browser):
        nodes = browser.visible()
        config = browser.config
        depths, labels, widths = {}, {}, {}
        for node in nodes:
            depth = depths[node.parent] + 1 if node.parent else 0
            depths[node] = depth
            labels[node] = shorten(node.label, config.max_label_width)
            widths[depth] = max(widths.get(depth, 0), cell_width(labels[node]))
        if config.orientation == "vertical":
            spans = {}
            centers = {}
            for node in reversed(nodes):
                children_width = (sum(spans[child] for child in node.children) +
                                  config.column_gap * (len(node.children) - 1)) if node.expanded and node.children else 0
                label_width = cell_width(labels[node])
                span = max(label_width, children_width)
                if node.expanded and node.children:
                    start = (span - children_width) // 2
                    preferred = (start + centers[node.children[0]] + start + children_width -
                                 spans[node.children[-1]] + centers[node.children[-1]]) // 2
                else:
                    preferred = label_width // 2
                centers[node] = max(label_width // 2, min(span - label_width + label_width // 2, preferred))
                spans[node] = span
            starts = {browser.root: 0}
            self.placements = []
            for node in nodes:
                self.placements.append(Placement(node, starts[node] + centers[node] - cell_width(labels[node]) // 2,
                                                 depths[node] * (config.row_spacing + 1), labels[node]))
                if node.expanded and node.children:
                    children_width = sum(spans[child] for child in node.children) + config.column_gap * (len(node.children) - 1)
                    start = starts[node] + (spans[node] - children_width) // 2
                    for child in node.children:
                        starts[child] = start
                        start += spans[child] + config.column_gap
            self.by_node = {item.node: item for item in self.placements}
            self.lines = []
            for item in self.placements:
                if not item.node.expanded or not item.node.children:
                    continue
                center = item.x + cell_width(item.label) // 2
                children = [self.by_node[child] for child in item.node.children]
                centers = [child.x + cell_width(child.label) // 2 for child in children]
                branch_y = children[0].y - 1
                self.lines.append((center, item.y, center, branch_y))
                self.lines.append((min(center, centers[0]), branch_y, max(center, centers[-1]), branch_y))
                self.lines.extend((x, branch_y, x, child.y) for x, child in zip(centers, children))
            self.width = max(item.x + cell_width(item.label) for item in self.placements)
            self.height = max(item.y for item in self.placements) + 1
            return
        columns = {0: 0}
        for depth in range(1, max(widths) + 1):
            columns[depth] = columns[depth - 1] + widths[depth - 1] + config.column_gap
        heights = {}
        for node in reversed(nodes):
            heights[node] = sum(heights[c] for c in node.children) if node.expanded and node.children else config.row_spacing
        rows = {browser.root: 0}
        self.placements = []
        self.by_node = {}
        for node in nodes:
            placement = Placement(node, columns[depths[node]], rows[node], labels[node])
            self.placements.append(placement)
            self.by_node[node] = placement
            if node.expanded:
                row = rows[node]
                for child in node.children:
                    rows[child] = row
                    row += heights[child]
        self.lines = []
        for item in self.placements:
            node = item.node
            if not node.expanded or not node.children:
                continue
            children = [self.by_node[c] for c in node.children]
            branch = children[0].x - config.column_gap + 2
            self.lines.append((item.x + cell_width(item.label), item.y, branch, item.y))
            self.lines.append((branch, item.y, branch, children[-1].y))
            for child in children:
                self.lines.append((branch, child.y, child.x - 1, child.y))
        self.width = max(p.x + cell_width(p.label) for p in self.placements)
        self.height = max(p.y for p in self.placements) + 1

    def line_cells(self, left, top, width, height):
        cells = {}
        # N/E/S/W bit mask; clipping preserves junctions at viewport edges.
        for x1, y1, x2, y2 in self.lines:
            if y1 == y2:
                if not top <= y1 < top + height:
                    continue
                for x in range(max(x1, left), min(x2, left + width - 1) + 1):
                    mask = (8 if x > x1 else 0) | (2 if x < x2 else 0)
                    cells[x, y1] = cells.get((x, y1), 0) | mask
            elif left <= x1 < left + width:
                for y in range(max(y1, top), min(y2, top + height - 1) + 1):
                    mask = (1 if y > y1 else 0) | (4 if y < y2 else 0)
                    cells[x1, y] = cells.get((x1, y), 0) | mask
        symbols = {0: "─", 1: "│", 2: "─", 4: "│", 8: "─", 5: "│", 10: "─",
                   3: "└", 6: "┌", 9: "┘", 12: "┐", 7: "├", 11: "┴", 13: "┤", 14: "┬", 15: "┼"}
        return {(x, y): symbols[m] for (x, y), m in cells.items()}


def plain_tree(browser):
    layout = Layout(browser)
    canvas = layout.line_cells(0, 0, layout.width, layout.height)
    lines = []
    for y in range(layout.height):
        items = sorted((p for p in layout.placements if p.y == y), key=lambda p: p.x)
        x, result = 0, ""
        for item in items:
            while x < item.x:
                result += canvas.get((x, y), " ")
                x += 1
            result += item.label
            x += cell_width(item.label)
        while x < layout.width:
            result += canvas.get((x, y), " ")
            x += 1
        lines.append(result.rstrip())
    return "\n".join(lines)
