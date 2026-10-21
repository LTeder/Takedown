"""Fetch the evaluation datasets into research/data/ and verify their checksums.

    uv run python fetch_data.py            # everything
    uv run python fetch_data.py vocadito   # one dataset
    uv run python fetch_data.py --force    # re-download even if verified

Each archive is checked against the size and MD5 that Zenodo publishes and the
SHA-256 pinned here, then unpacked into research/data/<name>/. research/data/
is gitignored; this script is the only record of what goes in it. Standard
library only, so it runs before `uv sync` if it has to.
"""

from __future__ import annotations

import argparse
import hashlib
import shutil
import sys
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath

DATA_DIR = Path(__file__).resolve().parent / "data"
CHUNK = 1 << 20


@dataclass(frozen=True)
class Dataset:
    name: str
    record: int
    file: str
    size: int
    md5: str
    sha256: str
    licence: str

    @property
    def url(self) -> str:
        return f"https://zenodo.org/api/records/{self.record}/files/{self.file}/content"


DATASETS = {
    d.name: d
    for d in [
        # Solo vocals with f0, note and lyric annotations (Bittner et al. 2021).
        Dataset(
            name="vocadito",
            record=5578259,
            file="vocadito.zip",
            size=58_503_561,
            md5="e066b6e1b977c77fe509155b53b9b2b2",
            sha256="9f5a067d97ebccf5caa511c1238c476be11051d1f609527e86c3702b37601f33",
            licence="CC BY 4.0",
        ),
        # Amateur Vocal Percussion (Delgado et al. 2019): beatbox with onset annotations.
        Dataset(
            name="avp",
            record=3245959,
            file="AVP_Dataset.zip",
            size=230_627_498,
            md5="f54cfec4660dd23eb0615302daa6f960",
            sha256="0515ab48833fe5f4cec8d37ec40c41e36490d55bdda68967f68efac04d264f50",
            licence="CC BY 4.0",
        ),
    ]
}


def hashes(path: Path) -> tuple[str, str]:
    md5, sha = hashlib.md5(), hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(CHUNK):
            md5.update(chunk)
            sha.update(chunk)
    return md5.hexdigest(), sha.hexdigest()


def verify(d: Dataset, path: Path) -> bool:
    if not path.is_file():
        return False
    size = path.stat().st_size
    if size != d.size:
        print(f"  {path.name}: size {size}, expected {d.size}")
        return False
    md5, sha = hashes(path)
    if md5 != d.md5:
        print(f"  {path.name}: md5 {md5}, expected {d.md5}")
        return False
    if sha != d.sha256:
        print(f"  {path.name}: sha256 {sha}, expected {d.sha256}")
        return False
    return True


def download(d: Dataset, dest: Path) -> None:
    part = dest.with_suffix(dest.suffix + ".part")
    print(f"  downloading {d.url}")
    with urllib.request.urlopen(d.url, timeout=60) as resp, part.open("wb") as out:
        done = 0
        while chunk := resp.read(CHUNK):
            out.write(chunk)
            done += len(chunk)
            print(f"\r  {done / 1e6:7.1f} / {d.size / 1e6:.1f} MB", end="", flush=True)
    print()
    part.replace(dest)


def extract(archive: Path, target: Path) -> None:
    """Unpack, refusing absolute paths and parent-directory escapes."""
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    with zipfile.ZipFile(archive) as z:
        for info in z.infolist():
            p = PurePosixPath(info.filename)
            if p.is_absolute() or ".." in p.parts:
                raise SystemExit(f"refusing unsafe path in {archive.name}: {info.filename}")
            # macOS resource forks add nothing but noise.
            if p.parts and p.parts[0] == "__MACOSX":
                continue
            z.extract(info, target)


def fetch(d: Dataset, force: bool) -> None:
    print(f"{d.name} (Zenodo {d.record}, {d.licence})")
    archives = DATA_DIR / "archives"
    archives.mkdir(parents=True, exist_ok=True)
    archive = archives / d.file
    target = DATA_DIR / d.name
    stamp = target / ".verified"

    if not force and stamp.is_file() and verify(d, archive):
        print("  already fetched and verified")
        return
    if force or not verify(d, archive):
        download(d, archive)
        if not verify(d, archive):
            raise SystemExit(f"{d.name}: checksum mismatch after download")
    extract(archive, target)
    stamp.write_text(f"{d.file} md5={d.md5} sha256={d.sha256}\n")
    print(f"  unpacked into {target.relative_to(DATA_DIR.parent)}")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("names", nargs="*", metavar="name", help=f"datasets to fetch: {', '.join(DATASETS)} (default: all)")
    ap.add_argument("--force", action="store_true", help="re-download even if a verified copy exists")
    args = ap.parse_args()
    if unknown := sorted(set(args.names) - set(DATASETS)):
        ap.error(f"unknown dataset {', '.join(unknown)}; choose from {', '.join(DATASETS)}")
    for name in args.names or DATASETS:
        fetch(DATASETS[name], args.force)


if __name__ == "__main__":
    sys.exit(main())
