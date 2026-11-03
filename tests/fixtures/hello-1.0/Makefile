PREFIX ?= /usr/local
DESTDIR ?=

CC      ?= cc
CFLAGS  ?= -O2 -Wall

all: hello

hello: hello.c
	$(CC) $(CFLAGS) -o hello hello.c

install: hello
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 0755 hello $(DESTDIR)$(PREFIX)/bin/hello

clean:
	rm -f hello

.PHONY: all install clean
