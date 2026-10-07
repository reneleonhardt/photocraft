using PhotoCraft.Psd;

if (args.Length != 2) { Console.Error.WriteLine("usage: inspect <input.psd> <output.psd>"); return 1; }
try
{
    using var document = Document.Parse(File.ReadAllBytes(args[0]), lazy: true);
    var header = document.Header;
    Console.WriteLine($"{header.Width} x {header.Height}, {header.Depth} bits, {header.LayerCount} layers");
    File.WriteAllBytes(args[1], document.ToBytes());
}
catch (Exception error)
{
    Console.Error.WriteLine(error.Message);
    return 1;
}
return 0;
