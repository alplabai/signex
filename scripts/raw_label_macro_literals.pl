#!/usr/bin/env perl
# Helper of scripts/check_raw_labels.sh (#2056): prints `file:line` for every
# bare string literal that is an element of a row![…], column![…] or stack![…]
# macro. In this iced fork `&str` is a widget, so `column!["Save"]` shows a
# literal that skips the house `text(raw(…))` / `text(t!(…))`. ast-grep cannot
# see macro arguments, so this reads them: it walks the brackets after each
# macro name, splits the top-level elements on commas, and flags an element
# that is nothing but one string literal. Nested macros are checked too.
#
# Skips comments and the `#[cfg(test)] mod …` tail of a file. A string that
# holds a bracket or comma is handled; a raw string r#"…"# is not (none in the
# UI crates; add it here if one appears).
use strict;
use warnings;

for my $file (@ARGV) {
    open(my $fh, '<', $file) or die "$file: $!";
    local $/;
    my $src = <$fh>;
    close $fh;
    $src =~ s/\r//g;
    $src =~ s/\n[ \t]*#\[cfg\(test\)\]\s*\n\s*mod\s.*\z/\n/s;

    while ($src =~ /\b(?:row|column|stack)!\s*\[/g) {
        my $i     = pos($src);
        my $depth = 0;
        my $start = $i;
        my @elements;
        my $n = length $src;
        while ($i < $n) {
            my $c = substr($src, $i, 1);
            if ($c eq '"') {
                $i++;
                while ($i < $n) {
                    my $d = substr($src, $i, 1);
                    if ($d eq '\\') { $i += 2; next; }
                    last if $d eq '"';
                    $i++;
                }
            } elsif (substr($src, $i) =~ /\A'(?:\\.[^']*|[^\\'])'/) {
                $i += length($&) - 1;
            } elsif (substr($src, $i, 2) eq '//') {
                $i++ while $i < $n && substr($src, $i, 1) ne "\n";
                next;
            } elsif ($c =~ /[\(\[\{]/) {
                $depth++;
            } elsif ($c =~ /[\)\}]/) {
                $depth--;
            } elsif ($c eq ']') {
                if ($depth == 0) {
                    push @elements, [$start, $i];
                    last;
                }
                $depth--;
            } elsif ($c eq ',' && $depth == 0) {
                push @elements, [$start, $i];
                $start = $i + 1;
            }
            $i++;
        }
        for my $span (@elements) {
            my ($from, $to) = @$span;
            my $body = substr($src, $from, $to - $from);
            $body =~ s{//[^\n]*}{}g;
            next unless $body =~ /\A(\s*)"(?:[^"\\]|\\.)*"\s*\z/s;
            my $lead = $1;
            my $line = (substr($src, 0, $from) =~ tr/\n//) + ($lead =~ tr/\n//) + 1;
            print "$file:$line\n";
        }
    }
}
