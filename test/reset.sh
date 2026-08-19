#!/usr/bin/env bash
# Limpa e recria a estrutura de teste do harbor.
set -e
cd "$(dirname "$0")"

rm -rf incoming downloads organized
mkdir -p incoming downloads organized

mk() {
    for f in "$@"; do
        echo "fake $f" > "incoming/$f"
    done
}

mk férias_2025.jpg wallpaper.png shot.gif logo.svg
mk relatorio.pdf livro.epub notas.txt tese.docx slides.pptx
mk linux.tar.gz source.zip fotos.7z backup.rar
mk playlist.mp3 album.flac beat.wav
mk trailer.mp4 clip.mkv vid.mov
mk setup.exe pacote.deb editor.AppImage
mk grande.part baixando.crdownload
mk aleatorio.xyz sem_extensao

echo "reset ok: $(ls incoming | wc -l) arquivos em incoming/, downloads/ e organized/ vazios"
