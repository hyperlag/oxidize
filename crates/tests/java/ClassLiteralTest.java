// ClassLiteralTest.java
// Tests class literal expressions (Foo.class) producing a JClass descriptor.
// Expected output (getName returns simple name in translated code):
//   String
//   Integer
//   int

class ClassLiteralTest {
    public static void main(String[] args) {
        Class<?> sc = String.class;
        System.out.println(sc.getSimpleName());

        Class<?> ic = Integer.class;
        System.out.println(ic.getSimpleName());

        Class<?> pc = int.class;
        System.out.println(pc.getSimpleName());
    }
}
