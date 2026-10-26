#!/bin/bash
cd "$(dirname "$0")"

clear
echo "======================================================="
echo "      GENERÁTOR ŠVÉDSKÝCH KŘÍŽOVEK PRO MACOS (cwgen)    "
echo "            Kompatibilní s Apple Silicon (M1/M2/M3/M4)  "
echo "======================================================="
echo ""

if ! command -v cargo &> /dev/null; then
    echo "CHYBA: Nástroj 'cargo' (Rust) nebyl v systému nalezen."
    echo ""
    echo "Na macOS ho snadno nainstalujete zadáním tohoto příkazu do Terminálu:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo ""
    echo "Stiskněte Enter pro ukončení..."
    read
    exit 1
fi

printf "Zadejte rozměr křížovky (např. 10x10, 15x15, 20x20, max 30x30) [výchozí: 12x12]: "
read ROZMER
if [ -z "$ROZMER" ]; then
    ROZMER="12x12"
fi

echo ""
echo "Vyberte preferovanou obtížnost definic:"
echo "  [1] Snadná (Easy)"
echo "  [2] Střední (Medium) [výchozí]"
echo "  [3] Těžká (Hard)"
echo "  [4] Libovolná / Mix"
printf "Zvolte obtížnost [1-4, výchozí: 2]: "
read OBT
if [ -z "$OBT" ]; then
    OBT="2"
fi

echo ""
printf "Zadejte tajenku (volitelné, Enter pro přeskočení): "
read TAJENKA

CMD_ARGS="--size $ROZMER -D $OBT"
if [ -n "$TAJENKA" ]; then
    CMD_ARGS="$CMD_ARGS -s $TAJENKA"
fi

echo ""
echo "Generuji křížovku... Čekejte prosím..."
echo ""

cargo run --release --bin cwgen -- $CMD_ARGS

STATUS=$?

if [ $STATUS -eq 0 ]; then
    echo ""
    echo "======================================================="
    echo " Křížovka byla úspěšně vygenerována do složky output/! "
    echo " Otevírám vygenerovanou HTML křížovku v prohlížeči...   "
    echo "======================================================="
    
    LATEST_HTML=$(ls -t output/*.html 2>/dev/null | head -n 1)
    if [ -n "$LATEST_HTML" ]; then
        open "$LATEST_HTML"
    fi
else
    echo ""
    echo "Při generování došlo k chybě."
fi

echo ""
echo "Stiskněte Enter pro zavření tohoto okna..."
read
