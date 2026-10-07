package main

import (
	"fmt"
	psd "github.com/storytold/photocraft/bindings/psd/go"
	"image/png"
	"os"
)

func run() error {
	if len(os.Args) != 4 {
		return fmt.Errorf("usage: inspect <input.psd> <output.psd> <preview.png>")
	}
	input, err := os.ReadFile(os.Args[1])
	if err != nil {
		return err
	}
	doc, err := psd.Parse(input, true)
	if err != nil {
		return err
	}
	defer doc.Close()
	header, err := doc.Header()
	if err != nil {
		return err
	}
	fmt.Printf("%d × %d, %d-bit, %d layers\n", header.Width, header.Height, header.Depth, header.LayerCount)
	data, err := doc.ToBytes()
	if err != nil {
		return err
	}
	if err := os.WriteFile(os.Args[2], data, 0644); err != nil {
		return err
	}
	preview, err := doc.Preview(256)
	if err != nil {
		return err
	}
	file, err := os.Create(os.Args[3])
	if err != nil {
		return err
	}
	err = png.Encode(file, preview)
	closeErr := file.Close()
	if err != nil {
		return err
	}
	return closeErr
}
func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
