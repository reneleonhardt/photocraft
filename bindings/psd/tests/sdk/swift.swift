import Foundation
import PhotoCraftPsd

let input = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
var document: OpaquePointer?
let status = input.withUnsafeBytes {
    pc_psd_parse_lazy($0.bindMemory(to: UInt8.self).baseAddress, $0.count, &document, nil)
}
guard status == PC_PSD_OK, let document else { throw CocoaError(.fileReadCorruptFile) }
defer { pc_psd_free(document) }
var header = pc_psd_header_info()
guard pc_psd_header(document, &header, nil) == PC_PSD_OK, header.width > 0, header.height > 0 else {
    throw CocoaError(.fileReadCorruptFile)
}
print("Swift C module consumer passed.")
