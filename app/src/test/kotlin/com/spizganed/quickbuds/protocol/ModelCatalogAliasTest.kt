package com.spizganed.quickbuds.protocol

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Test

class ModelCatalogAliasTest {
    private val air7 = ModelCatalog.Model(
        "064812",
        "realme Buds Air7",
        JSONObject("""{"aliases":["realme Buds 7"]}"""),
    )
    private val air7Pro = ModelCatalog.Model(
        "064C12",
        "realme Buds Air7 Pro",
        JSONObject("""{"aliases":["realme Buds 7 Pro"]}"""),
    )

    @Test fun requestedRealmeNamesResolveToTheirAir7Profiles() {
        val models = listOf(air7, air7Pro)

        assertEquals(air7, ModelCatalog.find(models, null, "realme Buds 7"))
        assertEquals(air7Pro, ModelCatalog.find(models, null, "realme Buds 7 Pro"))
        assertEquals(air7, ModelCatalog.find(models, null, "REALME BUDS 7"))
        assertEquals(air7Pro, ModelCatalog.find(models, "064C12", "realme Buds Air7 Pro"))
    }
}
