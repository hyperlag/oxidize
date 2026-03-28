class ThrowsDecl {
    static String risky(int x) throws IllegalArgumentException {
        if (x < 0) {
            throw new IllegalArgumentException("negative");
        }
        return "ok: " + x;
    }

    public static void main(String[] args) {
        try {
            System.out.println(risky(-1));
        } catch (IllegalArgumentException e) {
            System.out.println("caught: " + e.getMessage());
        }
        System.out.println(risky(5));
    }
}
