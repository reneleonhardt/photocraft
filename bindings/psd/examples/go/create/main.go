package main

import (
	"fmt"
	psd "github.com/storytold/photocraft/bindings/psd/go"
	"image"
	"os"
)

func run() error {
	if len(os.Args) != 2 {
		return fmt.Errorf("usage: create <output.psd>")
	}
	b, err := psd.NewBuilder(psd.DefaultConfig(1, 1))
	if err != nil {
		return err
	}
	defer b.Close()
	pixels := []byte{255, 0, 0, 128}
	if err := b.AddLayer("Red", image.Rect(0, 0, 1, 1), pixels, nil, nil); err != nil {
		return err
	}
	if err := b.Composite(pixels); err != nil {
		return err
	}
	if err := b.Resolution(300); err != nil {
		return err
	}
	doc, err := b.Build()
	if err != nil {
		return err
	}
	defer doc.Close()
	data, err := doc.ToBytes()
	if err != nil {
		return err
	}
	return os.WriteFile(os.Args[1], data, 0644)
}
func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
