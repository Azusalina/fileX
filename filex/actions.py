from .core import create_entry, rename, resolve_destination, transfer, trash


ARG_LABELS = {
    "new_file": "新文件名称", "new_folder": "新目录名称", "rename": "新名称",
    "copy": "复制到完整目标路径（相对路径以源文件父目录为基准）",
    "move": "移动到完整目标路径（相对路径以源文件父目录为基准）",
}


def perform(browser, action, argument=""):
    source = browser.selected.path
    if browser.selected is browser.root and action in {"rename", "move", "trash"}:
        raise ValueError("不能重命名、移动或删除当前根目录；请先浏览其父目录")
    if action in {"new_file", "new_folder"}:
        destination = create_entry(browser.target_directory(), argument, action == "new_folder")
        target = browser.selected if browser.selected.is_dir else browser.selected.parent
        browser.expand(target)
    elif action == "rename":
        destination = rename(source, argument)
    elif action in {"copy", "move"}:
        destination = transfer(source, resolve_destination(source, argument), action == "move")
    elif action == "trash":
        trash(source)
        destination = source.parent
    else:
        raise ValueError(f"未知操作: {action}")
    browser.refresh()
    browser.selected = next((n for n in browser.visible() if n.path == destination), browser.selected)
    return destination
