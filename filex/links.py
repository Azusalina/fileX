"""Bounded, on-demand link metadata; never build a global inode index."""
from pathlib import Path
import os
import stat


def peers(path, root, info, limit, maximum):
    paths, scanned, skipped, limited = [], 0, 0, False
    if stat.S_ISDIR(info.st_mode) or info.st_nlink <= 1:
        return paths, scanned, skipped, limited
    stack = []
    try:
        try:
            stack.append((os.scandir(root), 0))
        except OSError:
            return paths, 0, 1, False
        while stack:
            if scanned >= limit:
                limited = True
                break
            entries, depth = stack[-1]
            try:
                entry = next(entries)
            except StopIteration:
                entries.close()
                stack.pop()
                continue
            except OSError:
                scanned += 1
                skipped += 1
                continue
            scanned += 1
            try:
                current = entry.stat(follow_symlinks=False)
            except OSError:
                skipped += 1
                continue
            candidate = Path(entry.path)
            if (current.st_dev, current.st_ino) == (info.st_dev, info.st_ino) and candidate != path:
                paths.append(candidate)
                if len(paths) >= info.st_nlink - 1:
                    break
                if len(paths) >= maximum:
                    limited = True
                    break
            if stat.S_ISDIR(current.st_mode):
                if depth >= 128:
                    skipped += 1
                    continue
                try:
                    stack.append((os.scandir(candidate), depth + 1))
                except OSError:
                    skipped += 1
        return sorted(paths), scanned, skipped, limited
    finally:
        for entries, _ in stack:
            entries.close()


def details(path, root, info, config=None):
    path, root = Path(path), Path(root)
    data = {"link_count": info.st_nlink, "device": info.st_dev, "inode": info.st_ino,
            "directory": stat.S_ISDIR(info.st_mode), "hard_links": [],
            "hard_linked": not stat.S_ISDIR(info.st_mode) and info.st_nlink > 1}
    if stat.S_ISLNK(info.st_mode):
        try:
            target = path.readlink()
            data["symlink_target"] = str(target)
            data["symlink_absolute"] = str(target if target.is_absolute() else path.parent / target)
            try:
                path.stat()
                data["symlink_state"] = "有效"
            except (FileNotFoundError, NotADirectoryError):
                data["symlink_state"] = "断开"
            except OSError as error:
                data["symlink_state"] = f"无法访问: {error}"
        except OSError as error:
            data["symlink_error"] = str(error)
    if data["hard_linked"]:
        paths, scanned, skipped, limited = peers(path, root, info,
                                                config.scan_limit if config else 10000,
                                                config.max_results if config else 32)
        data.update(hard_links=[str(peer) for peer in paths], link_scanned=scanned,
                    link_skipped=skipped, link_limited=limited)
    return data


def details_text(data):
    # Import here to keep this helper independent of Browser's loading code.
    from .core import safe_text
    text = f"Links / 链接数: {data['link_count']}\nDevice / Inode: {data['device']} / {data['inode']}\n"
    if "symlink_target" in data:
        text += f"Soft link / 软链接 → {safe_text(data['symlink_target'])}\nTarget / 目标路径: {safe_text(data['symlink_absolute'])}\n状态: {safe_text(data['symlink_state'])}\n"
    if "symlink_error" in data:
        text += f"软链接目标无法读取: {safe_text(data['symlink_error'])}\n"
    if data["directory"]:
        text += "目录链接数含 . / 子目录 ..，不表示文件硬链接\n"
    elif data["hard_linked"]:
        text += f"Hard links / 硬链接: {data['link_count']} 个名称共享此 inode\n其他路径（当前根目录内）: {len(data['hard_links'])}\n"
        text += "".join(safe_text(peer) + "\n" for peer in data["hard_links"])
        if not data["hard_links"]:
            text += "未找到其他路径；可能位于根目录外或不可读\n"
        text += f"扫描 {data['link_scanned']} 项，跳过 {data['link_skipped']} 处" + ("，达到扫描 / 结果上限" if data["link_limited"] else "") + "\n"
    return text
