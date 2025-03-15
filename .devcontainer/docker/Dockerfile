FROM auser/development-base AS development

COPY ./scripts/zshrc /home/vscode/.zshrc

USER root
# Needed for testing
RUN apt-get update && apt-get install -y default-mysql-client


RUN apt-get update && apt-get install -y locales ca-certificates \
    && localedef -i en_US -c -f UTF-8 -A /usr/share/locale/locale.alias en_US.UTF-8

ENV LANG en_US.UTF-8
ENV LC_ALL en_US.UTF-8

USER vscode

CMD ["/usr/bin/zsh"]