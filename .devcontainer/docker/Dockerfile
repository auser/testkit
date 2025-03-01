FROM auser/development-base AS development

COPY ./scripts/zshrc /home/vscode/.zshrc

USER root
# Needed for testing
RUN apt-get update && apt-get install -y default-mysql-client

USER vscode

CMD ["/usr/bin/zsh"]