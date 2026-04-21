public class EnumMultipleConstructors {
    enum Mode {
        A(1),
        B("two"),
        C;

        Mode(int x) {}
        Mode(String s) {}
        Mode() {}
    }

    public static void main(String[] args) {
        System.out.println(Mode.A.name());
        System.out.println(Mode.B.name());
        System.out.println(Mode.C.name());
        System.out.println(Mode.values().length);
        System.out.println(Mode.B.ordinal());
        System.out.println(Mode.valueOf("C") == Mode.C);
    }
}
