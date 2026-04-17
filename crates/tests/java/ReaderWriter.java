// ReaderWriter.java
// Tests Reader/Writer as polymorphic types and BufferedReader(StringReader).
// Expected output:
//   hello
//   world
//   done

import java.io.*;

class ReaderWriter {
    public static void main(String[] args) throws Exception {
        // Test 1: Reader variable from StringReader, then wrap in BufferedReader
        Reader r = new StringReader("hello\nworld");
        BufferedReader br = new BufferedReader(r);
        System.out.println(br.readLine());  // hello
        System.out.println(br.readLine());  // world
        br.close();

        System.out.println("done");
    }
}
