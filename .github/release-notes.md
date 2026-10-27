Imprime capas de CD, DVD, Blu-ray, jogos e **VHS** no tamanho exato, com linhas de corte e dobra. Recriação para Linux do UnderCoverXP, com visual retrô e a mascote Disquinha.

## Instalação

### Arch Linux (e derivados)
Baixe o `undercovernc-*-x86_64.pkg.tar.zst` abaixo e instale:
```sh
sudo pacman -U undercovernc-*-x86_64.pkg.tar.zst
```

### Ubuntu / Debian (Ubuntu 22.04 ou mais novo)
Baixe o `undercovernc_*_amd64.deb` abaixo e instale (o apt puxa as dependências):
```sh
sudo apt install ./undercovernc_*_amd64.deb
```

### Qualquer distro (binário pronto, sem compilar)
```sh
tar -xzf undercovernc-*-linux-x86_64.tar.gz
cd undercovernc && ./install.sh
```
Instala em `~/.local` (sem sudo). Para remover: `./install.sh --uninstall`.

### Compilando o código-fonte
```sh
git clone https://github.com/notth4tcold/UnderCoverNc && cd UnderCoverNc
./install.sh
```
O script instala as dependências pelo pacman ou apt (pergunta antes) e o Rust pelo rustup, se precisar.

Depois de instalar, procure **UnderCoverNc** no menu de aplicativos ou rode `undercovernc`.
Para imprimir direto é preciso CUPS; sem ele dá para exportar PDF.

**Primeira impressão:** em *Arquivo › Configurar impressão*, imprima a **página de calibração** e confira com uma régua se o quadrado tem 100 × 100 mm.
