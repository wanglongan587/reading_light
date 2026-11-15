from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

root=Path(__file__).resolve().parent.parent
stage=root/'dist'/'windows-x64'
archive=root/'dist'/'glow-reading-windows-x64.zip'
# Registry license files can have Unix-epoch dates; ZIP timestamps start in 1980.
with ZipFile(archive,'w',compression=ZIP_DEFLATED,compresslevel=6,strict_timestamps=False) as output:
    for path in sorted(stage.rglob('*')):
        if path.is_file():
            output.write(path,path.relative_to(stage).as_posix())
print(archive)
